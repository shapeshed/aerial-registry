use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const CHANNELS_URL: &str = "https://somafm.com/channels.json";

#[derive(Deserialize)]
struct ChannelsResponse {
    channels: Vec<SomaFmChannel>,
}

#[derive(Deserialize)]
struct SomaFmChannel {
    id: String,
    title: String,
    description: String,
    genre: String,
    #[serde(default)]
    largeimage: Option<String>,
    playlists: Vec<SomaFmPlaylist>,
}

#[derive(Deserialize, Clone)]
struct SomaFmPlaylist {
    url: String,
    format: String,
    quality: String,
}

/// Highest-bitrate MP3 for broadest player compatibility; falls back to any
/// MP3 entry, then to whatever the channel lists first.
fn preferred_playlist(playlists: &[SomaFmPlaylist]) -> Option<&SomaFmPlaylist> {
    playlists
        .iter()
        .find(|p| p.format == "mp3" && p.quality == "highest")
        .or_else(|| playlists.iter().find(|p| p.format == "mp3"))
        .or_else(|| playlists.first())
}

/// Prefixed so the brand is clear wherever the name is shown, without every
/// downstream consumer (mood lists, search) needing its own override — unless
/// the channel's own title already has it (e.g. "SomaFM Live").
fn prefixed_name(title: String) -> String {
    if title.starts_with("SomaFM") {
        title
    } else {
        format!("SomaFM {title}")
    }
}

/// SomaFM's channel list only exposes `.pls` playlist wrappers, each listing
/// several redundant `ice*.somafm.com` mirrors for one stream — not directly
/// playable, so resolve to the first mirror at discovery time instead of
/// shipping a `.pls` URL the player can't handle.
fn parse_pls_stream_url(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("File1="))
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
}

async fn resolve_stream_url(client: &Client, playlist_url: &str) -> Option<String> {
    let text = client
        .get(playlist_url)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    parse_pls_stream_url(&text)
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "somafm", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let body: ChannelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "somafm", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let futures = body.channels.into_iter().map(|channel| {
        let client = client.clone();
        async move {
            let playlist_url = preferred_playlist(&channel.playlists).map(|p| p.url.clone());
            let stream_url = match playlist_url {
                Some(url) => resolve_stream_url(&client, &url).await,
                None => None,
            };
            (channel, stream_url)
        }
    });
    let results = futures::future::join_all(futures).await;

    let mut stations = Vec::new();
    for (channel, stream_url) in results {
        let Some(stream_url) = stream_url else {
            warn!(
                provider = "somafm",
                channel = %channel.id,
                "Could not resolve a stream URL — skipping"
            );
            continue;
        };

        let tags = channel
            .genre
            .split('|')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        let name = prefixed_name(channel.title);

        debug!(provider = "somafm", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url: channel.largeimage.filter(|u| !u.is_empty()),
            country: Some("United States".into()),
            country_code: Some("US".into()),
            tags,
            description: Some(channel.description).filter(|d| !d.is_empty()),
            provider: "somafm".into(),
            provider_id: Some(channel.id),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "somafm",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_file_entry_from_pls() {
        let pls = "[playlist]\nnumberofentries=2\nFile1=https://ice1.somafm.com/beatblender-128-mp3\nTitle1=x\nFile2=https://ice2.somafm.com/beatblender-128-mp3\n";
        assert_eq!(
            parse_pls_stream_url(pls).as_deref(),
            Some("https://ice1.somafm.com/beatblender-128-mp3")
        );
    }

    #[test]
    fn pls_with_no_file_entry_returns_none() {
        assert_eq!(
            parse_pls_stream_url("[playlist]\nnumberofentries=0\n"),
            None
        );
    }

    fn playlist(format: &str, quality: &str, url: &str) -> SomaFmPlaylist {
        SomaFmPlaylist {
            url: url.into(),
            format: format.into(),
            quality: quality.into(),
        }
    }

    #[test]
    fn prefers_highest_quality_mp3() {
        let playlists = vec![
            playlist("aac", "highest", "https://example.com/aac.pls"),
            playlist("mp3", "low", "https://example.com/mp3-low.pls"),
            playlist("mp3", "highest", "https://example.com/mp3-highest.pls"),
        ];
        assert_eq!(
            preferred_playlist(&playlists).unwrap().url,
            "https://example.com/mp3-highest.pls"
        );
    }

    #[test]
    fn falls_back_to_any_mp3_if_no_highest() {
        let playlists = vec![
            playlist("aac", "highest", "https://example.com/aac.pls"),
            playlist("mp3", "low", "https://example.com/mp3-low.pls"),
        ];
        assert_eq!(
            preferred_playlist(&playlists).unwrap().url,
            "https://example.com/mp3-low.pls"
        );
    }

    #[test]
    fn falls_back_to_first_entry_if_no_mp3() {
        let playlists = vec![playlist("aac", "high", "https://example.com/aac.pls")];
        assert_eq!(
            preferred_playlist(&playlists).unwrap().url,
            "https://example.com/aac.pls"
        );
    }

    #[test]
    fn genre_pipe_list_splits_into_multiple_tags() {
        let tags: Vec<String> = "ambient|electronic"
            .split('|')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        assert_eq!(tags, vec!["ambient".to_string(), "electronic".to_string()]);
    }

    #[test]
    fn prefixes_title_with_brand() {
        assert_eq!(
            prefixed_name("Beat Blender".to_string()),
            "SomaFM Beat Blender"
        );
    }

    #[test]
    fn does_not_double_prefix_a_title_that_already_has_it() {
        assert_eq!(prefixed_name("SomaFM Live".to_string()), "SomaFM Live");
    }
}

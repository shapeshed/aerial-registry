use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const CHANNELS_URL: &str = "https://fluxmusic.api.radiosphere.io/channels";

#[derive(Deserialize)]
struct ChannelsResponse {
    items: Vec<FluxChannel>,
}

#[derive(Deserialize)]
struct FluxChannel {
    #[serde(rename = "channelId")]
    channel_id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    summary: Option<String>,
    streams: Vec<FluxStream>,
    #[serde(rename = "coverImages", default)]
    cover_images: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
struct FluxStream {
    encoding: String,
    bitrate: u32,
    url: String,
}

/// Highest-bitrate MP3 for broadest player compatibility; falls back to any
/// MP3 entry, then to whatever the channel lists first.
fn preferred_stream(streams: &[FluxStream]) -> Option<&FluxStream> {
    streams
        .iter()
        .filter(|s| s.encoding == "mp3")
        .max_by_key(|s| s.bitrate)
        .or_else(|| streams.first())
}

/// Prefixed to match Radio Browser's own long-standing "FluxFM - <channel>"
/// naming convention for these same stations (confirmed against ~150
/// community-submitted duplicates) — not just branding, but so dedup's
/// trusted-vs-untrusted name match actually merges them instead of leaving
/// two differently-named copies of every channel in the registry. The
/// top-level "FluxFM" channel keeps its bare name (no "FluxFM - FluxFM").
fn prefixed_name(display_name: String) -> String {
    if display_name == "FluxFM" {
        display_name
    } else {
        format!("FluxFM - {display_name}")
    }
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "fluxfm", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let body: ChannelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "fluxfm", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for channel in body.items {
        let Some(stream) = preferred_stream(&channel.streams) else {
            continue;
        };
        let stream_url = stream.url.clone();
        let logo_url = channel
            .cover_images
            .as_ref()
            .and_then(|images| images.get("256_256.png"))
            .cloned();
        let name = prefixed_name(channel.display_name);

        debug!(provider = "fluxfm", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url,
            country: Some("Germany".into()),
            country_code: Some("DE".into()),
            tags: vec![],
            description: channel.summary.filter(|d| !d.is_empty()),
            provider: "fluxfm".into(),
            provider_id: Some(channel.channel_id),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "fluxfm",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(encoding: &str, bitrate: u32, url: &str) -> FluxStream {
        FluxStream {
            encoding: encoding.into(),
            bitrate,
            url: url.into(),
        }
    }

    #[test]
    fn prefers_highest_bitrate_mp3() {
        let streams = vec![
            stream("aac", 320, "https://example.com/aac-320"),
            stream("mp3", 64, "https://example.com/mp3-64"),
            stream("mp3", 320, "https://example.com/mp3-320"),
        ];
        assert_eq!(
            preferred_stream(&streams).unwrap().url,
            "https://example.com/mp3-320"
        );
    }

    #[test]
    fn falls_back_to_first_entry_if_no_mp3() {
        let streams = vec![stream("aac", 320, "https://example.com/aac-320")];
        assert_eq!(
            preferred_stream(&streams).unwrap().url,
            "https://example.com/aac-320"
        );
    }

    #[test]
    fn prefixes_display_name_with_brand() {
        assert_eq!(prefixed_name("80s".to_string()), "FluxFM - 80s");
    }

    #[test]
    fn prefixes_even_a_name_that_already_says_flux() {
        // Radio Browser's own convention prefixes every sub-channel, including
        // already-Flux-branded ones — matching that (not skipping it) is what
        // lets dedup merge the ~150 community duplicates under these names.
        assert_eq!(
            prefixed_name("FluxLounge".to_string()),
            "FluxFM - FluxLounge"
        );
        assert_eq!(
            prefixed_name("FluxFM Finest".to_string()),
            "FluxFM - FluxFM Finest"
        );
    }

    #[test]
    fn top_level_channel_keeps_its_bare_name() {
        assert_eq!(prefixed_name("FluxFM".to_string()), "FluxFM");
    }

    #[test]
    fn channels_response_parses_expected_shape() {
        let body = r#"{
            "items": [
                {
                    "channelId": "abc-123",
                    "name": "clubsandwich",
                    "displayName": "Clubsandwich",
                    "summary": "Some description",
                    "streams": [
                        { "name": "MP3-320", "encoding": "mp3", "bitrate": 320, "url": "https://example.com/stream.mp3" }
                    ],
                    "coverImage": "https://example.com/cover",
                    "coverImages": { "256_256.png": "https://example.com/cover-256.png" }
                }
            ]
        }"#;
        let parsed: ChannelsResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].channel_id, "abc-123");
        assert_eq!(
            parsed.items[0]
                .cover_images
                .as_ref()
                .unwrap()
                .get("256_256.png")
                .unwrap(),
            "https://example.com/cover-256.png"
        );
    }

    #[test]
    fn null_cover_images_parses_as_none() {
        // Real upstream data: a couple of channels ("listen-to-berlin",
        // "traumerei") have `"coverImages": null` instead of an object.
        let body = r#"{
            "items": [
                {
                    "channelId": "abc-123",
                    "name": "traumerei",
                    "displayName": "traumerei",
                    "summary": null,
                    "streams": [
                        { "name": "MP3-320", "encoding": "mp3", "bitrate": 320, "url": "https://example.com/stream.mp3" }
                    ],
                    "coverImage": null,
                    "coverImages": null
                }
            ]
        }"#;
        let parsed: ChannelsResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.items[0].cover_images.is_none());
    }
}

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const STATIONS_URL: &str = "https://api.mujrozhlas.cz/stations";
const COUNTRY: &str = "Czech Republic";
const COUNTRY_CODE: &str = "CZ";

#[derive(Deserialize)]
struct StationsResponse {
    data: Vec<StationItem>,
}

#[derive(Deserialize)]
struct StationItem {
    attributes: Attributes,
}

#[derive(Deserialize)]
struct Attributes {
    title: String,
    code: String,
    // The `logo` SVGs on mujrozhlas.cz are bot-blocked (403 for any client);
    // logoIcon is a square JPG on portal.rozhlas.cz that serves openly.
    #[serde(rename = "logoIcon")]
    logo_icon: Option<String>,
    #[serde(rename = "isAdHocStream", default)]
    is_ad_hoc_stream: bool,
    #[serde(rename = "audioLinks", default)]
    audio_links: Vec<AudioLink>,
}

#[derive(Deserialize)]
struct AudioLink {
    bitrate: u32,
    #[serde(rename = "linkType")]
    link_type: String,
    url: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(STATIONS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "cesky-rozhlas", "Failed to fetch stations: {e}");
            return vec![];
        }
    };

    let body: StationsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(
                provider = "cesky-rozhlas",
                "Failed to parse stations response: {e}"
            );
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for item in body.data {
        let attrs = item.attributes;
        // Ad-hoc streams are temporary event channels, not stations.
        if attrs.is_ad_hoc_stream {
            continue;
        }
        let Some(stream_url) = pick_stream(&attrs.audio_links) else {
            warn!(
                provider = "cesky-rozhlas",
                station = %attrs.code,
                "No direct stream listed — skipping"
            );
            continue;
        };
        debug!(provider = "cesky-rozhlas", name = %attrs.title, %stream_url, "Discovered station");
        stations.push(Station {
            name: attrs.title,
            stream_url,
            logo_url: attrs.logo_icon,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "cesky-rozhlas".into(),
            provider_id: Some(attrs.code),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "cesky-rozhlas",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Each station lists ~14 links: direct Icecast streams, `.m3u` playlist
/// wrappers ("livestream"), and DASH/HLS timeshift manifests. Playlist URLs
/// are against registry policy and timeshift needs DVR params, so take the
/// highest-bitrate direct stream (160kbps AAC for all but one station).
fn pick_stream(links: &[AudioLink]) -> Option<String> {
    links
        .iter()
        .filter(|l| l.link_type == "directstream")
        .max_by_key(|l| l.bitrate)
        .map(|l| l.url.clone())
}

#[cfg(test)]
mod tests {
    use super::{AudioLink, pick_stream};

    fn link(link_type: &str, bitrate: u32, url: &str) -> AudioLink {
        AudioLink {
            bitrate,
            link_type: link_type.to_string(),
            url: url.to_string(),
        }
    }

    #[test]
    fn picks_highest_bitrate_direct_stream() {
        let links = vec![
            link(
                "livestream",
                128,
                "https://rozhlas.stream/x_mp3_128.mp3.m3u",
            ),
            link("directstream", 64, "https://rozhlas.stream/x_low.aac"),
            link("directstream", 160, "https://rozhlas.stream/x_high.aac"),
            link(
                "timeshift",
                64,
                "https://wowza.radia.cz/x/playlist.m3u8?DVR",
            ),
        ];
        assert_eq!(
            pick_stream(&links).as_deref(),
            Some("https://rozhlas.stream/x_high.aac")
        );
    }

    #[test]
    fn no_direct_stream_means_no_station() {
        let links = vec![link("livestream", 128, "https://rozhlas.stream/x.m3u")];
        assert_eq!(pick_stream(&links), None);
    }
}

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const STATIONS_URL: &str = "https://apipr.polskieradio.pl/api/stacje";
const LOGO_BASE: &str = "https://player.polskieradio.pl/images";
const COUNTRY: &str = "Poland";
const COUNTRY_CODE: &str = "PL";

/// The stations API lists ~60 channels but carries no ids and no images.
/// Only the main networks have public logos (served by the official web
/// player), so the provider is scoped to those, matched by API name.
/// The remaining entries are thematic jukebox channels (Abba Non Stop,
/// Pink Floyd, …) that the no-logo policy would filter anyway.
const MAIN_STATIONS: &[(&str, &str, &str, &str)] = &[
    // (API name, display name, provider_id, player logo file)
    (
        "Jedynka",
        "Polskie Radio Jedynka",
        "jedynka",
        "jedynka-color-logo.png",
    ),
    (
        "Dwójka",
        "Polskie Radio Dwójka",
        "dwojka",
        "dwojka-color-logo.png",
    ),
    (
        "Trójka",
        "Polskie Radio Trójka",
        "trojka",
        "trojka-color-logo.png",
    ),
    (
        "Czwórka",
        "Polskie Radio Czwórka",
        "czworka",
        "czworka-color-logo.png",
    ),
    (
        "Polskie Radio 24",
        "Polskie Radio 24",
        "pr24",
        "pr24-color-logo.png",
    ),
    (
        "Radio Poland",
        "Radio Poland",
        "poland",
        "poland-color-logo.png",
    ),
    (
        "Radio Chopin",
        "Polskie Radio Chopin",
        "chopin",
        "chopin-color-logo.png",
    ),
    (
        "Radio Dzieciom",
        "Polskie Radio Dzieciom",
        "dzieciom",
        "dzieci-color-logo.png",
    ),
    (
        "Radio Kierowców",
        "Polskie Radio Kierowców",
        "kierowcow",
        "prk-color-logo.png",
    ),
];

#[derive(Deserialize)]
struct ApiStation {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Streams", default)]
    streams: Vec<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(STATIONS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "polskie-radio", "Failed to fetch stations: {e}");
            return vec![];
        }
    };

    let body: Vec<ApiStation> = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(
                provider = "polskie-radio",
                "Failed to parse stations response: {e}"
            );
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for (api_name, display_name, id, logo_file) in MAIN_STATIONS {
        let Some(api_station) = body.iter().find(|s| s.name == *api_name) else {
            warn!(
                provider = "polskie-radio",
                station = api_name,
                "Station missing from API"
            );
            continue;
        };
        let Some(stream_url) = pick_stream(&api_station.streams) else {
            warn!(
                provider = "polskie-radio",
                station = api_name,
                "No HLS stream listed"
            );
            continue;
        };
        debug!(provider = "polskie-radio", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: (*display_name).to_string(),
            stream_url,
            logo_url: Some(format!("{LOGO_BASE}/{logo_file}")),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "polskie-radio".into(),
            provider_id: Some((*id).to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "polskie-radio",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Each station lists ~8 stream variants. The Icecast MP3 ports are dead and
/// plain-HTTP HLS answers 502; only HLS over HTTPS serves, so pick the
/// `playlist.m3u8` variant and force the scheme.
fn pick_stream(streams: &[String]) -> Option<String> {
    streams
        .iter()
        .find(|u| u.ends_with("playlist.m3u8"))
        .map(|u| {
            if let Some(rest) = u.strip_prefix("http://") {
                format!("https://{rest}")
            } else {
                u.clone()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::pick_stream;

    #[test]
    fn picks_hls_and_forces_https() {
        let streams = vec![
            "rtmp://stream13.polskieradio.pl/pr3/pr3.sdp".to_string(),
            "http://mp3.polskieradio.pl:8904".to_string(),
            "http://stream13.polskieradio.pl/pr3/pr3.sdp/playlist.m3u8".to_string(),
        ];
        assert_eq!(
            pick_stream(&streams).as_deref(),
            Some("https://stream13.polskieradio.pl/pr3/pr3.sdp/playlist.m3u8")
        );
    }

    #[test]
    fn no_hls_means_no_station() {
        let streams = vec!["http://mp3.polskieradio.pl:8904".to_string()];
        assert_eq!(pick_stream(&streams), None);
    }
}

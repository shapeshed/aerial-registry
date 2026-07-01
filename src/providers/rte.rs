use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const LIVE_STATIONS_URL: &str = "https://www.rte.ie/radio/live_stations/json";
const MANIFEST_BASE: &str = "https://www.rte.ie/manifests";
const COUNTRY: &str = "Ireland";
const COUNTRY_CODE: &str = "IE";

#[derive(Deserialize)]
struct LiveStationsResponse {
    stations: Vec<RteStation>,
}

#[derive(Deserialize)]
struct RteStation {
    slug: String,
    name: String,
    #[serde(rename = "logoSvgUrl")]
    logo_svg_url: Option<String>,
    description: Option<String>,
}

/// The live_stations API uses "lyricfm" but the manifest is served at
/// "lyric" — "lyricfm.m3u8" 404s.
fn manifest_slug(api_slug: &str) -> &str {
    match api_slug {
        "lyricfm" => "lyric",
        other => other,
    }
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(LIVE_STATIONS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "rte", "Failed to fetch live stations: {e}");
            return vec![];
        }
    };

    let body: LiveStationsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(
                provider = "rte",
                "Failed to parse live stations response: {e}"
            );
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for rte_station in body.stations {
        let stream_url = format!("{MANIFEST_BASE}/{}.m3u8", manifest_slug(&rte_station.slug));
        let description = rte_station.description.filter(|d| !d.is_empty());

        debug!(provider = "rte", name = %rte_station.name, %stream_url, "Discovered station");
        stations.push(Station {
            name: rte_station.name,
            stream_url,
            logo_url: rte_station.logo_svg_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description,
            provider: "rte".into(),
            provider_id: Some(rte_station.slug),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rte",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

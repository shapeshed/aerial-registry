use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const BUNDLE_URL: &str = "https://orf.at/app-infos/sound/web/1.0/bundle.json?_o=sound.orf.at";

/// The two HLS quality tiers ORF's player uses ("q1a" low, "q2a" high) are
/// not documented anywhere — found by grepping the ORF Sound web app's JS
/// bundle for literal strings near its stream URL templates.
const STREAM_QUALITY: &str = "q2a";

#[derive(Deserialize)]
struct Bundle {
    stations: HashMap<String, StationEntry>,
}

#[derive(Deserialize)]
struct StationEntry {
    name: Option<String>,
    #[serde(rename = "liveStreamUrlTemplate")]
    live_stream_url_template: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(BUNDLE_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "orf", "Failed to fetch bundle: {e}");
            return vec![];
        }
    };

    let bundle: Bundle = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "orf", "Failed to parse bundle response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();

    for (slug, entry) in bundle.stations {
        // Entries with no liveStreamUrlTemplate are TV channels or the
        // on-demand archive, not radio stations — skip them.
        let Some(template) = entry.live_stream_url_template.filter(|t| !t.is_empty()) else {
            continue;
        };
        let Some(name) = entry.name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let stream_url = template.replace("{quality}", STREAM_QUALITY);

        debug!(provider = "orf", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url: None,
            country: Some("Austria".into()),
            country_code: Some("AT".into()),
            tags: vec![],
            description: None,
            provider: "orf".into(),
            provider_id: Some(slug),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "orf",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

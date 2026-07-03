use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// The on-air widget the rtvslo.si radio pages embed; the client_id is the
/// public one from the site's own page source.
const WIDGET_URL: &str = "https://api.rtvslo.si/preslikave/aktualno?client_id=82013fb3a531d5414f478747c1aca622&key=radio_onair_widget_v2";
const STREAM_BASE: &str = "https://mp3.rtvslo.si";
const COUNTRY: &str = "Slovenia";
const COUNTRY_CODE: &str = "SI";

/// The widget carries names and artwork but its getLiveStream API returns
/// HLS URLs signed with ~8h expiring tokens, useless in a static registry.
/// Streams come from the stable public Icecast mounts instead, mapped by
/// widget key. Radio Z is absent: it is digital-only behind the signed HLS
/// and has no Icecast mount.
const STATIONS: &[(&str, &str, &str)] = &[
    // (widget key, display name, icecast mount)
    ("ra1", "Radio Prvi", "ra1"),
    ("val202", "Val 202", "val202"),
    ("ars", "Radio Ars", "ars"),
    ("rakp", "Radio Koper", "rakp"),
    ("ramb", "Radio Maribor", "rmb"),
    ("capo", "Radio Capodistria", "capo"),
    ("mmr", "MMR", "mmr"),
    ("rsi", "Radio Si", "rsi"),
    ("202sport", "202 Sport", "sport202"),
];

#[derive(Deserialize)]
struct WidgetResponse {
    response: HashMap<String, WidgetStation>,
}

#[derive(Deserialize)]
struct WidgetStation {
    #[serde(rename = "overlay_icon")]
    overlay_icon: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(WIDGET_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "rtvslo", "Failed to fetch widget: {e}");
            return vec![];
        }
    };

    let body: WidgetResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "rtvslo", "Failed to parse widget response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for (key, display_name, mount) in STATIONS {
        let Some(widget) = body.response.get(*key) else {
            warn!(
                provider = "rtvslo",
                station = key,
                "Station missing from widget"
            );
            continue;
        };
        let stream_url = format!("{STREAM_BASE}/{mount}");
        debug!(provider = "rtvslo", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: (*display_name).to_string(),
            stream_url,
            logo_url: widget.overlay_icon.clone(),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "rtvslo".into(),
            provider_id: Some((*key).to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rtvslo",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_response_deserializes() {
        let json = r#"{"response":{"ra1":{"name":"PRVI","overlay_icon":"https://img.rtvslo.si/x.png","listeners":133}}}"#;
        let parsed: WidgetResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.response["ra1"].overlay_icon.as_deref(),
            Some("https://img.rtvslo.si/x.png")
        );
    }

    #[test]
    fn every_mapped_station_has_distinct_identity() {
        let mut keys: Vec<&str> = STATIONS.iter().map(|(k, _, _)| *k).collect();
        let mut mounts: Vec<&str> = STATIONS.iter().map(|(_, _, m)| *m).collect();
        keys.sort();
        keys.dedup();
        mounts.sort();
        mounts.dedup();
        assert_eq!(keys.len(), STATIONS.len());
        assert_eq!(mounts.len(), STATIONS.len());
    }
}

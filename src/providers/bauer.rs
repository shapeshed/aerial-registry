use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const COUNTRIES: &[(&str, &str, &str)] = &[
    (
        "GB",
        "United Kingdom",
        "https://listenapi.planetradio.co.uk/api9.2/stations/GB",
    ),
    (
        "IE",
        "Ireland",
        "https://listenapi.planetradio.co.uk/api9.2/stations/IE",
    ),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BauerStation {
    station_code: Option<String>,
    station_name: Option<String>,
    station_listen_bar_logo: Option<String>,
    station_streams: Option<Vec<BauerStream>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BauerStream {
    stream_url: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let mut stations = Vec::new();

    for (country_code, country_name, url) in COUNTRIES {
        let resp = match client.get(*url).send().await {
            Ok(r) => r,
            Err(e) => {
                error!(
                    provider = "bauer",
                    country = country_code,
                    "Failed to fetch stations: {e}"
                );
                continue;
            }
        };

        let raw: Vec<BauerStation> = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                error!(
                    provider = "bauer",
                    country = country_code,
                    "Failed to parse response: {e}"
                );
                continue;
            }
        };

        for s in raw {
            let name = match s.station_name.filter(|n| !n.is_empty()) {
                Some(n) => n,
                None => continue,
            };
            let stream_url = match s
                .station_streams
                .and_then(|streams| streams.into_iter().next())
                .and_then(|s| s.stream_url)
                .filter(|u| !u.is_empty())
            {
                Some(u) => u,
                None => continue,
            };

            debug!(provider = "bauer", %name, %stream_url, "Discovered station");
            stations.push(Station {
                name,
                stream_url,
                logo_url: s.station_listen_bar_logo.filter(|u| !u.is_empty()),
                country: Some(country_name.to_string()),
                country_code: Some(country_code.to_string()),
                tags: vec![],
                description: None,
                provider: "bauer".into(),
                provider_id: s.station_code.filter(|c| !c.is_empty()),
                trusted: true,
            });
        }
    }

    tracing::info!(
        provider = "bauer",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

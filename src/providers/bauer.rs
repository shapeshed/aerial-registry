use crate::http::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::{Station, StreamVariant};

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
    stream_type: Option<String>,
    stream_bit_rate: Option<u32>,
    #[serde(default)]
    stream_premium: bool,
    #[serde(default)]
    stream_users_only: bool,
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
            let streams = s.station_streams.unwrap_or_default();
            let Some(stream_url) = streams
                .iter()
                .find_map(|st| st.stream_url.clone().filter(|u| !u.is_empty()))
            else {
                continue;
            };
            // Public, non-HLS fixed-bitrate streams make good variants; the
            // premium/HD and HLS entries are left out.
            let variants: Vec<StreamVariant> = streams
                .iter()
                .filter(|st| !st.stream_premium && !st.stream_users_only)
                .filter(|st| matches!(st.stream_type.as_deref(), Some("adts") | Some("mp3")))
                .filter_map(|st| {
                    st.stream_url
                        .clone()
                        .filter(|u| !u.is_empty())
                        .map(|url| StreamVariant {
                            url,
                            bitrate: st.stream_bit_rate,
                            format: st.stream_type.clone(),
                        })
                })
                .collect();

            debug!(provider = "bauer", %name, %stream_url, "Discovered station");
            stations.push(Station {
                streams: if variants.len() > 1 {
                    variants
                } else {
                    Vec::new()
                },
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

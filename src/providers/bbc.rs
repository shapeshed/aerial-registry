use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const NETWORKS_URL: &str = "https://rms.api.bbc.co.uk/v2/networks?limit=100";
const MANIFEST_UUID: &str = "3441A116-B12E-4D2F-ACA8-C1984642FA4B";

#[derive(Deserialize)]
struct NetworksResponse {
    data: Vec<Network>,
}

#[derive(Deserialize)]
struct Network {
    default_service_id: Option<String>,
    title: Option<String>,
    titles: Option<Titles>,
}

#[derive(Deserialize)]
struct Titles {
    primary: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(NETWORKS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "bbc", "Failed to fetch networks: {e}");
            return vec![];
        }
    };

    let body: NetworksResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "bbc", "Failed to parse networks response: {e}");
            return vec![];
        }
    };

    // Fan out all manifest fetches concurrently: 65 networks × 2 variants = ~130 requests.
    let fetches: Vec<_> = body
        .data
        .into_iter()
        .filter_map(|network| {
            let service_id = network.default_service_id.filter(|s| !s.is_empty())?;
            let name = network
                .title
                .filter(|s| !s.is_empty())
                .or_else(|| network.titles.and_then(|t| t.primary).filter(|s| !s.is_empty()))
                .unwrap_or_else(|| service_id_to_name(&service_id));
            let logo_url = format!(
                "https://sounds.files.bbci.co.uk/3.9.4/networks/{service_id}/colour_default.svg"
            );
            Some((service_id, name, logo_url))
        })
        .flat_map(|(service_id, name, logo_url)| {
            [("uk", ""), ("nonuk", " (International)")].map(|(variant, suffix)| {
                let client = client.clone();
                let service_id = service_id.clone();
                let name = name.clone();
                let logo_url = logo_url.clone();
                async move {
                    let manifest_url = format!(
                        "https://a.files.bbci.co.uk/ms6/live/{MANIFEST_UUID}/audio/simulcast/hls/{variant}/pc_hd_abr_v2/ak/{service_id}.m3u8"
                    );
                    let resolved = resolve_stream_url(&client, &manifest_url).await;
                    (service_id, name, logo_url, suffix, resolved)
                }
            })
        })
        .collect();

    let results = join_all(fetches).await;

    let mut stations = Vec::new();
    for (service_id, name, logo_url, label_suffix, resolved) in results {
        match resolved {
            Some(stream_url) => {
                let station_name = format!("{name}{label_suffix}");
                debug!(provider = "bbc", name = %station_name, %stream_url, "Discovered station");
                stations.push(Station {
                    name: station_name,
                    stream_url,
                    logo_url: Some(logo_url),
                    country: Some("United Kingdom".into()),
                    country_code: Some("GB".into()),
                    tags: vec![],
                    description: None,
                    provider: "bbc".into(),
                    provider_id: Some(service_id.clone()),
                    trusted: true,
                });
            }
            None => {
                warn!(provider = "bbc", %service_id, variant = label_suffix, "No stream resolved — skipping");
            }
        }
    }

    tracing::info!(
        provider = "bbc",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Resolves a stream URL to its canonical playback URL.
///
/// For HLS manifests (.m3u8): fetches the manifest and extracts the first CDN
/// stream URL from within it. Returns None if the manifest is unreachable (e.g.
/// a nonuk stream that doesn't exist for this station).
///
/// For direct streams (Icecast, MP3, AAC): returns the URL unchanged — there is
/// no manifest to parse, the URL itself is the stream.
pub async fn resolve_stream_url(client: &Client, url: &str) -> Option<String> {
    if !url.ends_with(".m3u8") {
        return Some(url.to_owned());
    }
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    text.lines()
        .map(str::trim)
        .find(|l| l.starts_with("http"))
        .map(str::to_owned)
}

fn service_id_to_name(id: &str) -> String {
    let stripped = id.strip_prefix("bbc_").unwrap_or(id);
    let words: Vec<String> = stripped
        .split('_')
        .map(|word| match word {
            "bbc" => "BBC".into(),
            "fm" => "FM".into(),
            "mw" => "MW".into(),
            "lw" => "LW".into(),
            "am" => "AM".into(),
            "1xtra" => "1Xtra".into(),
            "6music" => "6 Music".into(),
            "wm" => "WM".into(),
            w => {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().to_string() + c.as_str(),
                }
            }
        })
        .collect();
    let name = words.join(" ");
    if name.starts_with("BBC") {
        name
    } else {
        format!("BBC {name}")
    }
}

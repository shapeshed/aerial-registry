use std::collections::HashMap;

use crate::http::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "Brazil";
const COUNTRY_CODE: &str = "BR";
const PROVIDER: &str = "ebc";

/// EBC (Empresa Brasil de Comunicação) runs Brazil's public radio: the Rádio
/// Nacional network and Rádio MEC. Its sites are Plone, and each station is an
/// `Emissora` content object whose REST representation carries the HLS
/// `stream_url` — so the station list is discovered from the broadcaster's own
/// API rather than catalogued. The search endpoint returns the station fields
/// directly, including the image scales used for artwork.
const SITES: &[&str] = &[
    "https://radionacional.ebc.com.br",
    "https://radiomec.ebc.com.br",
];
const SEARCH: &str = "++api++/@search?portal_type=Emissora&b_size=100&metadata_fields=stream_url";

#[derive(Deserialize)]
struct SearchResponse {
    items: Vec<Emissora>,
}

#[derive(Deserialize)]
struct Emissora {
    #[serde(rename = "@id")]
    id: Option<String>,
    title: Option<String>,
    stream_url: Option<String>,
    image_scales: Option<ImageScales>,
}

#[derive(Deserialize)]
struct ImageScales {
    image: Option<Vec<ImageEntry>>,
}

#[derive(Deserialize)]
struct ImageEntry {
    download: Option<String>,
    scales: Option<HashMap<String, Scale>>,
}

#[derive(Deserialize)]
struct Scale {
    download: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let mut stations = Vec::new();
    for site in SITES {
        let url = format!("{site}/{SEARCH}");
        let response = match client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(resp) => match resp.error_for_status() {
                Ok(resp) => resp.json::<SearchResponse>().await,
                Err(e) => {
                    error!(provider = PROVIDER, site, "Search request failed: {e}");
                    continue;
                }
            },
            Err(e) => {
                error!(provider = PROVIDER, site, "Search request failed: {e}");
                continue;
            }
        };

        match response {
            Ok(SearchResponse { items }) => {
                for item in items {
                    match station_from(item) {
                        Some(station) => {
                            debug!(provider = PROVIDER, name = %station.name, url = %station.stream_url, "Discovered station");
                            stations.push(station);
                        }
                        None => warn!(
                            provider = PROVIDER,
                            site, "Skipped emissora with no stream URL"
                        ),
                    }
                }
            }
            Err(e) => error!(
                provider = PROVIDER,
                site, "Could not parse search response: {e}"
            ),
        }
    }

    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

fn station_from(item: Emissora) -> Option<Station> {
    let name = item.title?.trim().to_string();
    let stream_url = item.stream_url?.trim().to_string();
    if name.is_empty() || !stream_url.starts_with("http") {
        return None;
    }

    let id = item.id.unwrap_or_else(|| stream_url.clone());
    let provider_id = id.trim_end_matches('/').rsplit('/').next().unwrap_or(&id);
    Some(Station {
        name,
        stream_url,
        logo_url: logo_url(&id, item.image_scales.as_ref()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: PROVIDER.into(),
        provider_id: Some(provider_id.to_string()),
        trusted: true,
    })
}

/// Build an absolute logo URL from Plone's image scales (the download paths are
/// relative to the object).
fn logo_url(id: &str, scales: Option<&ImageScales>) -> Option<String> {
    let entry = scales?.image.as_ref()?.first()?;
    let scale_download = entry
        .scales
        .as_ref()
        .and_then(|s| s.get("preview").or_else(|| s.get("thumb")))
        .and_then(|s| s.download.as_deref())
        .or(entry.download.as_deref())?;
    Some(format!("{}/{scale_download}", id.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::{Emissora, station_from};

    #[test]
    fn builds_station_with_logo_from_image_scales() {
        let json = r#"{
            "@id": "https://radionacional.ebc.com.br/emissoras/radio-nacional-do-rio-de-janeiro",
            "title": "Rádio Nacional do Rio de Janeiro",
            "stream_url": "https://radionacionalrio-stream.ebc.com.br/ebc/radionacionalriodejaneiro/playlist.m3u8",
            "image_scales": { "image": [ { "download": "@@images/image-563-x.png",
                "scales": { "preview": { "download": "@@images/image-400-y.png" } } } ] }
        }"#;
        let item: Emissora = serde_json::from_str(json).unwrap();
        let station = station_from(item).unwrap();
        assert_eq!(station.name, "Rádio Nacional do Rio de Janeiro");
        assert_eq!(
            station.stream_url,
            "https://radionacionalrio-stream.ebc.com.br/ebc/radionacionalriodejaneiro/playlist.m3u8"
        );
        assert_eq!(
            station.logo_url.as_deref(),
            Some(
                "https://radionacional.ebc.com.br/emissoras/radio-nacional-do-rio-de-janeiro/@@images/image-400-y.png"
            )
        );
        assert_eq!(
            station.provider_id.as_deref(),
            Some("radio-nacional-do-rio-de-janeiro")
        );
        assert!(station.trusted);
    }
}

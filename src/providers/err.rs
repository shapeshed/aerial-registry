use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// Each ERR radio channel's site serves the player API the apps use; the
/// live stream comes from `playerInit.live.src`. Should the API fail, the
/// stable `live.err.ee/live/<id>.m3u8` pattern is the fallback (all five
/// verified live).
const COUNTRY: &str = "Estonia";
const COUNTRY_CODE: &str = "EE";

/// Logos are the channels' own branding cards on ERR's photo CDN —
/// content-addressed crops whose URLs are immutable (Raadio Tallinn's has
/// been stable since 2014).
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (site subdomain, provider_id and stream id, display name, logo)
    (
        "vikerraadio",
        "vikerraadio",
        "Vikerraadio",
        "https://s.err.ee/photo/crop/2023/11/23/2160406h6dcft6.jpg",
    ),
    (
        "r2",
        "raadio2",
        "Raadio 2",
        "https://s.err.ee/photo/crop/2023/11/23/2160463h7092t6.jpg",
    ),
    (
        "klassikaraadio",
        "klassikaraadio",
        "Klassikaraadio",
        "https://s.err.ee/photo/crop/2020/03/07/758890hd478t6.png",
    ),
    (
        "r4",
        "raadio4",
        "Raadio 4",
        "https://s.err.ee/photo/crop/2026/04/01/3286149h31a5t6.jpg",
    ),
    (
        "raadiotallinn",
        "raadiotallinn",
        "Raadio Tallinn",
        "https://s.err.ee/photo/crop/2014/04/01/215972hb3c2t6.jpg",
    ),
];

#[derive(Deserialize)]
struct PageData {
    #[serde(rename = "pageControlData")]
    page_control_data: PageControlData,
}

#[derive(Deserialize)]
struct PageControlData {
    #[serde(rename = "playerInit")]
    player_init: PlayerInit,
}

#[derive(Deserialize)]
struct PlayerInit {
    live: Live,
}

#[derive(Deserialize)]
struct Live {
    src: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches: Vec<_> = STATIONS
        .iter()
        .map(|(subdomain, id, display_name, logo_url)| {
            let client = client.clone();
            async move {
                let url = format!(
                    "https://{subdomain}.err.ee/api/radio/getRadioPageData?contentId=0&parentContentId=0&radiomanUrl=&mediaId=0"
                );
                let stream = match client.get(&url).send().await {
                    Ok(r) => match r.json::<PageData>().await {
                        Ok(d) => normalise_src(&d.page_control_data.player_init.live.src),
                        Err(e) => {
                            error!(provider = "err", station = id, "Failed to parse page data: {e}");
                            None
                        }
                    },
                    Err(e) => {
                        error!(provider = "err", station = id, "Failed to fetch page data: {e}");
                        None
                    }
                };
                (*id, *display_name, *logo_url, stream)
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (id, display_name, logo_url, stream) in join_all(fetches).await {
        let stream_url = stream.unwrap_or_else(|| {
            warn!(
                provider = "err",
                station = id,
                "Using fallback stream pattern"
            );
            format!("https://live.err.ee/live/{id}.m3u8")
        });
        debug!(provider = "err", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url: Some(logo_url.to_string()),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "err".into(),
            provider_id: Some(id.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "err",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// The API emits protocol-relative sources (`//live.err.ee/...`).
fn normalise_src(src: &str) -> Option<String> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }
    if let Some(rest) = src.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    src.starts_with("https://").then(|| src.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalise_src;

    #[test]
    fn normalises_protocol_relative_sources() {
        assert_eq!(
            normalise_src("//live.err.ee/live/vikerraadio.m3u8").as_deref(),
            Some("https://live.err.ee/live/vikerraadio.m3u8")
        );
        assert_eq!(
            normalise_src("https://live.err.ee/live/raadio2.m3u8").as_deref(),
            Some("https://live.err.ee/live/raadio2.m3u8")
        );
        assert_eq!(normalise_src(""), None);
        assert_eq!(normalise_src("http://insecure.example/x.m3u8"), None);
    }
}

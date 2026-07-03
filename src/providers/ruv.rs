use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// RÚV's player resolves each channel's stream through a geo endpoint that
/// returns the current CDN URL (and a geoblock flag — false for all radio
/// channels). Used as the live source so the registry tracks CDN moves.
const GEO_BASE: &str = "https://geo.spilari.ruv.is/channel";
const COUNTRY: &str = "Iceland";
const COUNTRY_CODE: &str = "IS";

/// Rondó is absent from the geo resolver (and from the player's GraphQL
/// Channels enum), but its stream exists at the same Akamai pattern as the
/// others — hence the static fallback per channel.
const STATIONS: &[(&str, &str, &str)] = &[
    // (slug: geo resolver, stream pattern and provider_id; name; Commons PNG logo)
    (
        "ras1",
        "RÚV Rás 1",
        "https://upload.wikimedia.org/wikipedia/commons/thumb/5/59/R%C3%A1s_1_2019_logo.svg/960px-R%C3%A1s_1_2019_logo.svg.png",
    ),
    (
        "ras2",
        "RÚV Rás 2",
        "https://upload.wikimedia.org/wikipedia/commons/thumb/9/93/R%C3%A1s_2_2019_logo.svg/960px-R%C3%A1s_2_2019_logo.svg.png",
    ),
    (
        "rondo",
        "RÚV Rondó",
        "https://upload.wikimedia.org/wikipedia/commons/thumb/1/1f/Rond%C3%B3_2019_logo.svg/960px-Rond%C3%B3_2019_logo.svg.png",
    ),
];

#[derive(Deserialize)]
struct GeoResponse {
    url: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches: Vec<_> = STATIONS
        .iter()
        .map(|(slug, display_name, logo_url)| {
            let client = client.clone();
            async move {
                let stream_url = resolve_stream(&client, slug).await;
                (*slug, *display_name, *logo_url, stream_url)
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (slug, display_name, logo_url, stream_url) in join_all(fetches).await {
        debug!(provider = "ruv", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url: Some(logo_url.to_string()),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "ruv".into(),
            provider_id: Some(slug.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "ruv",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn resolve_stream(client: &Client, slug: &str) -> String {
    let fallback = fallback_stream(slug);
    match client.get(format!("{GEO_BASE}/{slug}")).send().await {
        Ok(resp) => match resp.json::<GeoResponse>().await {
            Ok(GeoResponse { url: Some(url) }) if !url.is_empty() => url,
            _ => {
                warn!(
                    provider = "ruv",
                    station = slug,
                    "Geo resolver had no URL; using fallback"
                );
                fallback
            }
        },
        Err(e) => {
            error!(
                provider = "ruv",
                station = slug,
                "Geo resolver failed: {e}; using fallback"
            );
            fallback
        }
    }
}

fn fallback_stream(slug: &str) -> String {
    format!("https://ruv-radio-live.akamaized.net/streymi/{slug}/{slug}.m3u8")
}

#[cfg(test)]
mod tests {
    use super::fallback_stream;

    #[test]
    fn fallback_follows_akamai_pattern() {
        assert_eq!(
            fallback_stream("rondo"),
            "https://ruv-radio-live.akamaized.net/streymi/rondo/rondo.m3u8"
        );
    }
}

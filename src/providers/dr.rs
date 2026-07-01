use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const CHANNELS_URL: &str = "https://api.dr.dk/radio/v5/channels";
const IMAGE_BASE_URL: &str = "https://api.dr.dk/radio/v2/images";

/// Internal monitoring/test feeds the API returns alongside real public
/// stations (master control room monitors, a webcam audio feed, and their
/// duplicate aliases) — not real listener-facing channels.
const EXCLUDED_SLUGS: &[&str] = &[
    "mcrweb1", "mcrweb2", "dr-web-2", "dr-web-3", "dr-web-4", "dr-web-8", "p3webcam",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Channel {
    slug: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    audio_assets: Vec<AudioAsset>,
    #[serde(default)]
    channel_logos: Vec<ImageAsset>,
    #[serde(default)]
    image_assets: Vec<ImageAsset>,
    #[serde(default)]
    districts: Vec<Channel>,
}

#[derive(Deserialize)]
struct AudioAsset {
    format: String,
    url: String,
}

#[derive(Deserialize)]
struct ImageAsset {
    id: String,
    ratio: String,
}

/// Prefer HLS; fall back to whatever the API lists first (an ICY/MP3 stream).
fn pick_stream(assets: &[AudioAsset]) -> Option<String> {
    assets
        .iter()
        .find(|a| a.format == "HLS")
        .or_else(|| assets.first())
        .map(|a| a.url.clone())
}

/// `channelLogos` is a dedicated square logo field present on most channels;
/// `imageAssets` sometimes carries a square (1:1) crop too as a fallback.
fn pick_logo(channel_logos: &[ImageAsset], image_assets: &[ImageAsset]) -> Option<String> {
    channel_logos
        .iter()
        .chain(image_assets.iter())
        .find(|i| i.ratio == "1:1")
        .map(|i| format!("{IMAGE_BASE_URL}/{}?ratio=1:1", i.id))
}

fn station_from_channel(channel: &Channel) -> Option<Station> {
    let stream_url = pick_stream(&channel.audio_assets)?;
    let logo_url = pick_logo(&channel.channel_logos, &channel.image_assets);
    let description = channel.description.clone().filter(|d| !d.is_empty());

    debug!(provider = "dr", name = %channel.title, %stream_url, "Discovered station");
    Some(Station {
        name: channel.title.clone(),
        stream_url,
        logo_url,
        country: Some("Denmark".into()),
        country_code: Some("DK".into()),
        tags: vec![],
        description,
        provider: "dr".into(),
        provider_id: Some(channel.slug.clone()),
        trusted: true,
    })
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "dr", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let channels: Vec<Channel> = match resp.json().await {
        Ok(c) => c,
        Err(e) => {
            error!(provider = "dr", "Failed to parse channels response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();

    for channel in &channels {
        if EXCLUDED_SLUGS.contains(&channel.slug.as_str()) {
            continue;
        }

        // P4 and P5 are containers for regional district stations rather
        // than directly playable channels — emit one station per district.
        if !channel.districts.is_empty() {
            for district in &channel.districts {
                match station_from_channel(district) {
                    Some(station) => stations.push(station),
                    None => {
                        warn!(provider = "dr", slug = %district.slug, "District has no stream — skipping")
                    }
                }
            }
            continue;
        }

        if let Some(station) = station_from_channel(channel) {
            stations.push(station);
        }
    }

    tracing::info!(
        provider = "dr",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

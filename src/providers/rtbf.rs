use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const CHANNELS_URL: &str = "https://bff-service.rtbf.be/oaos/v1.6/channels?_sort%5Bid%5D=asc&types=radio%2Cwebradio&_limit=500&platform=WEB&excludeIds=132%2C140";
const COUNTRY: &str = "Belgium";
const COUNTRY_CODE: &str = "BE";

#[derive(Deserialize)]
struct ChannelsResponse {
    data: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    key: String,
    label: String,
    tagline: Option<String>,
    #[serde(rename = "streamUrl")]
    stream_url: Option<StreamUrl>,
    #[serde(rename = "logoFlat")]
    logo_flat: Option<LogoVariants>,
    logo: Option<LogoVariants>,
}

#[derive(Deserialize)]
struct StreamUrl {
    aac: Option<String>,
    mp3: Option<String>,
}

#[derive(Deserialize)]
struct LogoVariants {
    light: Option<LogoFormats>,
}

#[derive(Deserialize)]
struct LogoFormats {
    png: Option<String>,
}

/// Only 5 flagship stations have a square `logoFlat`; every other station
/// only has the wide wordmark `logo`, used here as a fallback so every
/// station has some logo rather than none.
fn pick_logo(logo_flat: Option<LogoVariants>, logo: Option<LogoVariants>) -> Option<String> {
    logo_flat
        .and_then(|l| l.light)
        .and_then(|l| l.png)
        .or_else(|| logo.and_then(|l| l.light).and_then(|l| l.png))
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "rtbf", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let body: ChannelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "rtbf", "Failed to parse channels response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for channel in body.data {
        let Some(stream_url) = channel
            .stream_url
            .and_then(|s| s.aac.filter(|u| !u.is_empty()).or(s.mp3))
        else {
            continue;
        };
        let logo_url = pick_logo(channel.logo_flat, channel.logo);
        let description = channel.tagline.filter(|t| !t.is_empty());

        debug!(provider = "rtbf", name = %channel.label, %stream_url, "Discovered station");
        stations.push(Station {
            name: channel.label,
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description,
            provider: "rtbf".into(),
            provider_id: Some(channel.key),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rtbf",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

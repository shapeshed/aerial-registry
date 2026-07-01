use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const CHANNELS_URL: &str = "https://fos.sbs.com.au/web/audio/channels";
const COUNTRY: &str = "Australia";
const COUNTRY_CODE: &str = "AU";

/// SBS's own "Front Of Site" service has no endpoint that lists all channel
/// IDs — these Brightspot CMS UUIDs were found by inspecting each channel
/// page's server-rendered data. An eighth channel (SBS EuroPop / Sounds of
/// Home, HLS slug "sbs4") has a live stream but no findable current page or
/// ID, so it's excluded.
const CHANNEL_IDS: &[&str] = &[
    "00000183-abaa-db73-ab83-ffbf5e740000", // SBS Chill
    "00000183-abac-d32e-a3cb-bbffa66c0000", // SBS PopAsia
    "00000183-ab9e-d32e-a3cb-bbdfda660000", // SBS Radio 1
    "00000183-aba0-d32e-a3cb-bbff0b7d0000", // SBS Radio 2
    "00000183-aba2-db73-ab83-ffbf96a40000", // SBS Radio 3
    "00000183-abae-da02-a9df-fbbf27f30000", // SBS South Asian
    "00000183-abaf-db73-ab83-ffbfdec20000", // SBS Arabic24
];

#[derive(Deserialize)]
struct Channel {
    #[serde(rename = "epgId")]
    epg_id: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "streamUrl")]
    stream_url: String,
    #[serde(rename = "leadImage")]
    lead_image: Option<LeadImage>,
}

#[derive(Deserialize)]
struct LeadImage {
    attributes: ImageAttributes,
}

#[derive(Deserialize)]
struct ImageAttributes {
    sizes: Vec<ImageSize>,
}

#[derive(Deserialize)]
struct ImageSize {
    name: String,
    src: String,
}

fn pick_logo(lead_image: Option<LeadImage>) -> Option<String> {
    lead_image
        .and_then(|i| i.attributes.sizes.into_iter().find(|s| s.name == "1x1"))
        .map(|s| s.src)
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let ids = CHANNEL_IDS.join(",");
    let resp = match client
        .get(CHANNELS_URL)
        .query(&[("ids", &ids)])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "sbs", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let channels: Vec<Channel> = match resp.json().await {
        Ok(c) => c,
        Err(e) => {
            error!(provider = "sbs", "Failed to parse channels response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for channel in channels {
        let logo_url = pick_logo(channel.lead_image);
        let description = channel.description.filter(|d| !d.is_empty());

        debug!(provider = "sbs", name = %channel.name, stream_url = %channel.stream_url, "Discovered station");
        stations.push(Station {
            name: channel.name,
            stream_url: channel.stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description,
            provider: "sbs".into(),
            provider_id: Some(channel.epg_id),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "sbs",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

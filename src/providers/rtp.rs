use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "Portugal";
const COUNTRY_CODE: &str = "PT";

/// RTP has no public endpoint that enumerates all radio channel IDs — this list
/// was assembled by probing `livechannelonair.php` across RTP's channel ID space
/// (it's shared with TV channels, filtered here to the `radio` type).
const CHANNEL_IDS: &[u32] = &[
    91, 92, 1, 94, 95, 97, 98, 99, 100, 101, 102, 103, 104,
];

#[derive(Deserialize)]
struct OnAirResponse {
    raw: RawResult,
}

#[derive(Deserialize)]
struct RawResult {
    result: Vec<ChannelInfo>,
}

#[derive(Deserialize)]
struct ChannelInfo {
    channel_name: Option<String>,
    channel_summary: Option<String>,
    channel_card_logo: Option<String>,
    channel_rewrite: Option<String>,
    channel_type: Option<String>,
    stream_url: Option<StreamUrl>,
}

#[derive(Deserialize)]
struct StreamUrl {
    http: Option<HttpUrls>,
}

#[derive(Deserialize)]
struct HttpUrls {
    standard: Option<String>,
}

async fn fetch_channel(client: &Client, id: u32) -> Option<Station> {
    let url = format!(
        "https://www.rtp.pt/play/livechannelonair.php?channel={id}&howmanynext=1&howmanybefore=0&channeltype=radio"
    );
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: OnAirResponse = resp.json().await.ok()?;
    let channel = body.raw.result.into_iter().next()?;

    if !channel.channel_type.as_deref().unwrap_or_default().starts_with("radio") {
        warn!(provider = "rtp", channel_id = id, "Channel ID is no longer a radio channel — skipping");
        return None;
    }

    let name = channel.channel_name?;
    let stream_url = channel.stream_url?.http?.standard?;
    let description = channel
        .channel_summary
        .map(|s| s.replace('\u{a0}', " ").trim().to_string())
        .filter(|s| !s.is_empty());

    debug!(provider = "rtp", %name, %stream_url, "Discovered station");
    Some(Station {
        name,
        stream_url,
        logo_url: channel.channel_card_logo.filter(|s| !s.is_empty()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description,
        provider: "rtp".into(),
        provider_id: channel.channel_rewrite,
        trusted: true,
    })
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches = CHANNEL_IDS.iter().map(|&id| fetch_channel(client, id));
    let stations: Vec<Station> = join_all(fetches).await.into_iter().flatten().collect();

    if stations.len() < CHANNEL_IDS.len() {
        error!(
            provider = "rtp",
            found = stations.len(),
            expected = CHANNEL_IDS.len(),
            "Some RTP channel IDs failed to resolve"
        );
    }

    tracing::info!(provider = "rtp", count = stations.len(), "Discovery complete");
    stations
}

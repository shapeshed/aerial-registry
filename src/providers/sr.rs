use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const CHANNELS_URL: &str = "https://api.sr.se/api/v2/channels?format=json&pagination=false";

#[derive(Deserialize)]
struct ChannelsResponse {
    channels: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    id: u64,
    name: String,
    tagline: Option<String>,
    image: Option<String>,
    liveaudio: Option<LiveAudio>,
}

#[derive(Deserialize)]
struct LiveAudio {
    url: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "sr", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let body: ChannelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "sr", "Failed to parse channels response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();

    for channel in body.channels {
        let Some(stream_url) = channel
            .liveaudio
            .and_then(|a| a.url)
            .filter(|u| !u.is_empty())
        else {
            continue;
        };
        let description = channel.tagline.filter(|t| !t.is_empty());

        debug!(provider = "sr", name = %channel.name, %stream_url, "Discovered station");
        stations.push(Station {
            name: channel.name,
            stream_url,
            logo_url: channel.image,
            country: Some("Sweden".into()),
            country_code: Some("SE".into()),
            tags: vec![],
            description,
            provider: "sr".into(),
            provider_id: Some(channel.id.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "sr",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

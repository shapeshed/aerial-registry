use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const LIVE_STREAMS_URL: &str = "https://www.cbc.ca/listen/api/v1/live-radio/live-streams";
const COUNTRY: &str = "Canada";
const COUNTRY_CODE: &str = "CA";

#[derive(Deserialize)]
struct StreamsResponse {
    data: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    #[serde(rename = "fullTitle")]
    full_title: String,
    description: Option<String>,
    #[serde(rename = "streamUrl")]
    stream_url: String,
    media: Media,
    network: Network,
}

#[derive(Deserialize)]
struct Media {
    #[serde(rename = "callSign")]
    call_sign: String,
}

#[derive(Deserialize)]
struct Network {
    #[serde(rename = "logoUrl")]
    logo_url: Option<String>,
}

/// CBC's English (`cbc.ca/listen`) and French (`ici.radio-canada.ca/ohdio`)
/// services share the same Akamai stream infrastructure but have entirely
/// separate web stacks. This live-streams endpoint only covers English CBC
/// Radio One and CBC Music. French Radio-Canada (ICI Première, ICI Musique)
/// has no equivalent station-directory endpoint — its stream URLs are only
/// resolvable per-station via services.radio-canada.ca/media/validation/v2
/// given a numeric idMedia, and no public listing of those IDs by region
/// exists, so it's out of scope here.
pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(LIVE_STREAMS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "cbc", "Failed to fetch live streams: {e}");
            return vec![];
        }
    };

    let body: StreamsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(
                provider = "cbc",
                "Failed to parse live streams response: {e}"
            );
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for channel in body.data {
        let description = channel.description.filter(|d| !d.is_empty());

        debug!(provider = "cbc", name = %channel.full_title, stream_url = %channel.stream_url, "Discovered station");
        stations.push(Station {
            name: channel.full_title,
            stream_url: channel.stream_url,
            logo_url: channel.network.logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description,
            provider: "cbc".into(),
            provider_id: Some(channel.media.call_sign),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "cbc",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

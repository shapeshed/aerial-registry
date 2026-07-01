use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const GRAPHQL_URL: &str = "https://api.nporadio.nl/graphql";
const QUERY: &str = "query { core_channels { data { id name slug } } }";
const ICECAST_BASE: &str = "https://icecast.omroep.nl";
const COUNTRY: &str = "Netherlands";
const COUNTRY_CODE: &str = "NL";

/// NPO Radio's GraphQL API only exposes station metadata (name/slug) — the
/// actual playback URL requires a separate player-token + stream-link flow
/// (prod.npoplayer.nl) that is geo-restricted to the Netherlands and returns
/// HTTP 451 from anywhere else, making it unusable for a registry built
/// outside the country. This instead uses NPO's older Icecast infrastructure
/// (icecast.omroep.nl, now CNAMEd to icecast.npocloud.nl), which is still
/// live and not geo-restricted, confirmed working for the slugs below.
///
/// Newer channels — NPO Soul & Jazz, NPO Sterren NL, NPO Campus, FunX Fissa,
/// FunX Afro — are not on this legacy infrastructure and have no other
/// unrestricted stream source, so they are deliberately not included.
const ICECAST_SLUGS: &[(&str, &str)] = &[
    ("npo-radio-1", "radio1-bb-mp3"),
    ("npo-radio-2", "radio2-bb-mp3"),
    ("npo-3fm", "3fm-bb-mp3"),
    ("npo-radio-4", "radio4-bb-mp3"),
    ("npo-radio-5", "radio5-bb-mp3"),
    ("npo-blend", "npoblend-bb-mp3"),
    ("npo-funx", "funx-bb-mp3"),
    ("npo-funx-amsterdam", "funx-amsterdam-bb-mp3"),
    ("npo-funx-denhaag", "funx-denhaag-bb-mp3"),
    ("npo-funx-rotterdam", "funx-rotterdam-bb-mp3"),
    ("npo-funx-utrecht", "funx-utrecht-bb-mp3"),
    ("npo-funx-arab", "funx-arab-bb-mp3"),
    ("npo-funx-hiphop", "funx-hiphop-bb-mp3"),
    ("npo-funx-latin", "funx-latin-bb-mp3"),
    ("npo-funx-slowjamz", "funx-slowjamz-bb-mp3"),
];

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
}

#[derive(Deserialize)]
struct GqlData {
    core_channels: ChannelsPagination,
}

#[derive(Deserialize)]
struct ChannelsPagination {
    data: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    slug: String,
    name: String,
}

fn icecast_slug(graphql_slug: &str) -> Option<&'static str> {
    ICECAST_SLUGS
        .iter()
        .find(|(slug, _)| *slug == graphql_slug)
        .map(|(_, icecast)| *icecast)
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = serde_json::json!({ "query": QUERY });
    let resp = match client.post(GRAPHQL_URL).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "npo", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let gql: GqlResponse = match resp.json().await {
        Ok(g) => g,
        Err(e) => {
            error!(provider = "npo", "Failed to parse channels response: {e}");
            return vec![];
        }
    };

    let channels = match gql.data {
        Some(d) => d.core_channels.data,
        None => {
            error!(provider = "npo", "No data in response");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for channel in channels {
        let Some(icecast) = icecast_slug(&channel.slug) else {
            continue;
        };
        let stream_url = format!("{ICECAST_BASE}/{icecast}");

        debug!(provider = "npo", name = %channel.name, %stream_url, "Discovered station");
        stations.push(Station {
            name: channel.name,
            stream_url,
            logo_url: None,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "npo".into(),
            provider_id: Some(channel.slug),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "npo",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

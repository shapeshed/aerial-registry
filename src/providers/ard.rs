use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const GRAPHQL_URL: &str = "https://api.ardaudiothek.de/graphql";

const QUERY: &str = "{ permanentLivestreams(first: 200) { nodes { id title audios { url mimeType } image { url } publicationService { genre } } } }";

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlData {
    permanent_livestreams: Option<Livestreams>,
}

#[derive(Deserialize)]
struct Livestreams {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    id: String,
    title: String,
    audios: Vec<Audio>,
    image: Option<Image>,
    publication_service: Option<PublicationService>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Audio {
    url: String,
    mime_type: String,
}

#[derive(Deserialize)]
struct Image {
    url: String,
}

#[derive(Deserialize)]
struct PublicationService {
    genre: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = serde_json::json!({ "query": QUERY });

    let resp = match client.post(GRAPHQL_URL).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "ard", "Failed to fetch stations: {e}");
            return vec![];
        }
    };

    let gql: GqlResponse = match resp.json().await {
        Ok(g) => g,
        Err(e) => {
            error!(provider = "ard", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let nodes = match gql.data.and_then(|d| d.permanent_livestreams) {
        Some(ls) => ls.nodes,
        None => {
            error!(provider = "ard", "No livestream data in response");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for node in nodes {
        // Prefer HLS for adaptive streaming, fallback to first available audio
        let stream_url = node
            .audios
            .iter()
            .find(|a| a.mime_type == "application/vnd.apple.mpegurl")
            .or_else(|| node.audios.first())
            .map(|a| a.url.clone());

        let stream_url = match stream_url {
            Some(u) => u,
            None => continue,
        };

        let logo_url = node
            .image
            .map(|img| img.url.replace("{width}", "500"));

        let tags = node
            .publication_service
            .as_ref()
            .and_then(|ps| ps.genre.as_deref())
            .filter(|g| !g.is_empty())
            .map(|g| vec![g.to_string()])
            .unwrap_or_default();

        debug!(provider = "ard", name = %node.title, %stream_url, "Discovered station");
        stations.push(Station {
            name: node.title,
            stream_url,
            logo_url,
            country: Some("Germany".to_string()),
            country_code: Some("DE".to_string()),
            tags,
            description: None,
            provider: "ard".into(),
            provider_id: Some(node.id),
            trusted: true,
        });
    }

    tracing::info!(provider = "ard", count = stations.len(), "Discovery complete");
    stations
}

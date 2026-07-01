use reqwest::Client;
use serde::Deserialize;
use tracing::warn;

use crate::station::Station;

const STATIONS_TOML: &str = include_str!("../../stations.toml");

#[derive(Deserialize)]
struct TomlFile {
    stations: Vec<TomlStation>,
}

#[derive(Deserialize)]
struct TomlStation {
    name: String,
    country_code: String,
    stream_url: String,
    logo_url: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
}

pub async fn discover(_client: &Client) -> Vec<Station> {
    let file: TomlFile = match toml::from_str(STATIONS_TOML) {
        Ok(f) => f,
        Err(e) => {
            warn!(error = %e, "Failed to parse stations.toml");
            return vec![];
        }
    };

    let stations: Vec<Station> = file
        .stations
        .into_iter()
        .map(|s| Station {
            name: s.name,
            stream_url: s.stream_url,
            logo_url: s.logo_url,
            country: None,
            country_code: Some(s.country_code),
            tags: s.tags.unwrap_or_default(),
            description: s.description,
            provider: "curated".into(),
            provider_id: None,
            trusted: false,
        })
        .collect();

    tracing::info!(
        provider = "curated",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

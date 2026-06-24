use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::station::Station;

const MAX_CONCURRENT: usize = 20;
const MAX_RETRIES: u32 = 3;
const RETRY_BASE_MS: u64 = 500;

// Tags that describe logistics, geography, or brand rather than genre/format.
const NOISE: &[&str] = &[
    "radio",
    "bbc",
    "uk",
    "england",
    "scotland",
    "wales",
    "ireland",
    "london",
    "fm",
    "am",
    "dab",
    "dab+",
    "hd",
    "internet",
    "web",
    "online",
    "stream",
    "streaming",
    "live",
    "public",
    "europe",
    "national",
    "local",
    "regional",
    "digital",
    "broadcast",
    "english",
    "station",
];

#[derive(Deserialize)]
struct RbServer {
    name: String,
}

#[derive(Deserialize)]
struct RbStation {
    tags: String,
    votes: u32,
    clickcount: u32,
}

pub async fn enrich(client: &Client, mut stations: Vec<Station>) -> Vec<Station> {
    let servers = match discover_servers(client).await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Radio Browser server discovery failed — skipping tag enrichment");
            return stations;
        }
    };

    info!(servers = servers.len(), primary = %servers[0], "Radio Browser servers discovered");

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));

    let lookups: Vec<_> = stations
        .iter()
        .map(|station| {
            let client = client.clone();
            let sem = semaphore.clone();
            let servers = servers.clone();
            let name = search_name(&station.name);
            let country_code = station.country_code.clone();
            async move {
                let _permit = sem.acquire().await.unwrap();
                fetch_tags_with_retry(&client, &servers, &name, country_code.as_deref()).await
            }
        })
        .collect();

    let tag_results = join_all(lookups).await;

    let mut enriched = 0usize;
    for (station, new_tags) in stations.iter_mut().zip(tag_results) {
        if let Some(tags) = new_tags {
            if !tags.is_empty() {
                for tag in tags {
                    if !station.tags.contains(&tag) {
                        station.tags.push(tag);
                    }
                }
                enriched += 1;
            }
        }
    }

    info!(enriched, total = stations.len(), "Tag enrichment complete");
    stations
}

async fn discover_servers(client: &Client) -> anyhow::Result<Vec<String>> {
    let servers: Vec<RbServer> = client
        .get("https://all.api.radio-browser.info/json/servers")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("request failed: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse failed: {e}"))?;

    if servers.is_empty() {
        anyhow::bail!("server list was empty");
    }

    Ok(servers.into_iter().map(|s| s.name).collect())
}

/// Retries across servers with exponential backoff: 500ms, 1s, 2s.
async fn fetch_tags_with_retry(
    client: &Client,
    servers: &[String],
    name: &str,
    country_code: Option<&str>,
) -> Option<Vec<String>> {
    for attempt in 0..MAX_RETRIES {
        // Rotate through available servers on each retry.
        let server = &servers[attempt as usize % servers.len()];

        match fetch_tags(client, server, name, country_code).await {
            Ok(tags) => return tags,
            Err(e) => {
                let delay_ms = RETRY_BASE_MS * 2u64.pow(attempt);
                warn!(
                    %name,
                    %server,
                    attempt = attempt + 1,
                    delay_ms,
                    error = %e,
                    "Radio Browser lookup failed, retrying"
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }

    warn!(%name, "Radio Browser lookup exhausted retries — no tags");
    None
}

async fn fetch_tags(
    client: &Client,
    server: &str,
    name: &str,
    country_code: Option<&str>,
) -> anyhow::Result<Option<Vec<String>>> {
    let url = format!("https://{server}/json/stations/search");
    let mut query = vec![
        ("name", name.to_owned()),
        ("limit", "5".into()),
        ("hidebroken", "true".into()),
        ("order", "votes".into()),
        ("reverse", "true".into()),
    ];
    if let Some(cc) = country_code {
        query.push(("countrycode", cc.to_owned()));
    }

    let results: Vec<RbStation> = client
        .get(&url)
        .query(&query)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("request: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("status: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse: {e}"))?;

    let best = results
        .into_iter()
        .filter_map(|s| {
            let tags = clean_tags(&s.tags);
            if tags.is_empty() {
                return None;
            }
            let score = tags.len() * 1000 + s.votes as usize + s.clickcount as usize;
            Some((tags, score))
        })
        .max_by_key(|(_, score)| *score);

    if let Some((ref tags, _)) = best {
        debug!(%name, ?tags, "Enriched tags");
    }

    Ok(best.map(|(tags, _)| tags))
}

fn search_name(name: &str) -> String {
    name.trim_end_matches(" (International)").trim().to_owned()
}

fn clean_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .filter(|t| t.is_ascii())
        .filter(|t| !NOISE.contains(&t.as_str()))
        .filter(|t| t.len() >= 3)
        .filter(|t| !(t.contains('.') && t.chars().any(|c| c.is_ascii_digit())))
        .collect()
}

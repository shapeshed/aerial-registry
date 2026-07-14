use std::sync::Arc;

use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::radio_browser_client::{discover_servers, with_retry};
use crate::station::Station;

const MAX_CONCURRENT: usize = 20;

#[derive(Deserialize)]
struct RbStation {
    tags: String,
    votes: u32,
    clickcount: u32,
}

pub async fn enrich(client: &Client, mut stations: Vec<Station>) -> Vec<Station> {
    // Tag enrichment is one Radio Browser lookup per station; skip it for
    // faster local test builds. Tags fall back to whatever providers supply.
    if std::env::var("AERIAL_SKIP_ENRICH").is_ok_and(|v| !v.is_empty() && v != "0") {
        info!(
            total = stations.len(),
            "Radio Browser tag enrichment skipped (AERIAL_SKIP_ENRICH)"
        );
        return stations;
    }

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
            // The bulk radio-browser provider already carries its own
            // authoritative tags straight from this same API — re-querying
            // it by name here would be redundant load on an upstream that's
            // already prone to 502s.
            let skip = station.provider == "radio-browser";
            let name = search_name(&station.name);
            let country_code = station.country_code.clone();
            async move {
                if skip {
                    return None;
                }
                let _permit = sem.acquire().await.unwrap();
                fetch_tags_with_retry(&client, &servers, &name, country_code.as_deref()).await
            }
        })
        .collect();

    let tag_results = join_all(lookups).await;

    let mut enriched = 0usize;
    for (station, new_tags) in stations.iter_mut().zip(tag_results) {
        if let Some(tags) = new_tags
            && !tags.is_empty()
        {
            for tag in tags {
                if !station.tags.contains(&tag) {
                    station.tags.push(tag);
                }
            }
            enriched += 1;
        }
    }

    info!(enriched, total = stations.len(), "Tag enrichment complete");
    stations
}

/// Retries across servers with exponential backoff: 500ms, 1s, 2s.
async fn fetch_tags_with_retry(
    client: &Client,
    servers: &[String],
    name: &str,
    country_code: Option<&str>,
) -> Option<Vec<String>> {
    with_retry(servers, name, |server| async move {
        fetch_tags(client, &server, name, country_code).await
    })
    .await
    .flatten()
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

// Radio Browser tags are free-form; only tags that map into the registry
// taxonomy survive, so every published station's tags conform whether or not
// the AI overlay has assessed it yet.
fn clean_tags(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in raw.split(',') {
        if !tag.is_ascii() {
            continue;
        }
        if let Some(tag) = super::tags::normalize_tag(tag)
            && !out.contains(&tag)
        {
            out.push(tag);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::clean_tags;

    #[test]
    fn maps_free_form_tags_into_taxonomy() {
        assert_eq!(
            clean_tags("Rap, classic hits, jazz, RAP"),
            vec!["hip-hop".to_string(), "jazz".to_string()]
        );
    }

    #[test]
    fn drops_noise_and_unknown_genres() {
        assert!(clean_tags("radio, fm, online, webradio, schlager, 128kbps").is_empty());
    }
}

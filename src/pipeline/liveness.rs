use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::station::Station;

const MAX_CONCURRENT: usize = 50;
const TIMEOUT_SECS: u64 = 10;

pub async fn check(client: &reqwest::Client, stations: Vec<Station>) -> Vec<Station> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let client = client.clone();
    let total = stations.len();

    let tasks: Vec<_> = stations
        .into_iter()
        .map(|station| {
            let sem = semaphore.clone();
            let client = client.clone();
            tokio::spawn(async move {
                if station.trusted {
                    return Some(station);
                }
                let _permit = sem.acquire().await.unwrap();
                if is_live(&client, &station.stream_url).await {
                    Some(station)
                } else {
                    None
                }
            })
        })
        .collect();

    let mut live = Vec::new();
    for task in tasks {
        if let Ok(Some(station)) = task.await {
            live.push(station);
        }
    }

    let removed = total - live.len();
    tracing::info!(total, removed, live = live.len(), "Liveness check complete");
    live
}

async fn is_live(client: &reqwest::Client, url: &str) -> bool {
    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);

    // Try HEAD first. Any non-success response (including connection errors,
    // timeouts, and 4xx) falls through to GET — many Icecast servers don't
    // support HEAD at all and return inconsistent errors.
    let head_ok = match tokio::time::timeout(timeout, client.head(url).send()).await {
        Ok(Ok(resp)) => {
            let s = resp.status();
            if s.is_success() || s.is_redirection() {
                debug!(url, %s, "Stream live (HEAD)");
                return true;
            }
            false
        }
        _ => false,
    };
    let _ = head_ok;

    // GET fallback: we only need the response status, not the body.
    // reqwest won't download the body until we call .bytes()/.text(), so this
    // is cheap even for infinite audio streams.
    match tokio::time::timeout(timeout, client.get(url).send()).await {
        Ok(Ok(resp)) => {
            let s = resp.status();
            let ok = s.is_success() || s.is_redirection();
            if ok {
                debug!(url, %s, "Stream live (GET)");
            } else {
                warn!(url, %s, "Stream not live");
            }
            ok
        }
        Ok(Err(e)) => {
            warn!(url, error = %e, "Stream unreachable");
            false
        }
        Err(_) => {
            warn!(url, "Stream timed out");
            false
        }
    }
}

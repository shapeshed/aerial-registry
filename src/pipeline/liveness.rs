use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::station::Station;

const MAX_CONCURRENT: usize = 50;
const TIMEOUT_SECS: u64 = 10;

struct LivenessResult {
    station: Option<Station>,
    checked: bool,
    upgraded: bool,
    removed_reason: Option<RemoveReason>,
}

#[derive(Clone, Copy)]
enum RemoveReason {
    Unreachable,
    ProtocolRedirect,
    UnsupportedScheme,
}

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
                let mut station = station;
                if station.trusted {
                    return LivenessResult {
                        station: Some(station),
                        checked: false,
                        upgraded: false,
                        removed_reason: None,
                    };
                }

                let _permit = sem.acquire().await.unwrap();
                match live_stream_url(&client, &station.stream_url).await {
                    Ok(live_url) => {
                        let upgraded = live_url != station.stream_url;
                        if upgraded {
                            info!(
                                from = %station.stream_url,
                                to = %live_url,
                                "Upgraded stream URL to HTTPS"
                            );
                            station.stream_url = live_url;
                        }
                        LivenessResult {
                            station: Some(station),
                            checked: true,
                            upgraded,
                            removed_reason: None,
                        }
                    }
                    Err(reason) => LivenessResult {
                        station: None,
                        checked: true,
                        upgraded: false,
                        removed_reason: Some(reason),
                    },
                }
            })
        })
        .collect();

    let mut live = Vec::new();
    let mut checked = 0usize;
    let mut upgraded = 0usize;
    let mut unreachable = 0usize;
    let mut protocol_redirect = 0usize;
    let mut unsupported_scheme = 0usize;

    for task in tasks {
        if let Ok(result) = task.await {
            if result.checked {
                checked += 1;
            }
            if result.upgraded {
                upgraded += 1;
            }
            match result.removed_reason {
                Some(RemoveReason::Unreachable) => unreachable += 1,
                Some(RemoveReason::ProtocolRedirect) => protocol_redirect += 1,
                Some(RemoveReason::UnsupportedScheme) => unsupported_scheme += 1,
                None => {}
            }
            if let Some(station) = result.station {
                live.push(station);
            }
        }
    }

    let removed = total - live.len();
    tracing::info!(
        total,
        checked,
        skipped_trusted = total - checked,
        upgraded,
        removed,
        removed_unreachable = unreachable,
        removed_protocol_redirect = protocol_redirect,
        removed_unsupported_scheme = unsupported_scheme,
        live = live.len(),
        "Liveness check complete"
    );
    live
}

async fn live_stream_url(client: &reqwest::Client, url: &str) -> Result<String, RemoveReason> {
    if let Some(candidate) = https_candidate(url) {
        if let Some(resolved) = probe_live_url(client, &candidate, false).await {
            if resolved.starts_with("https://") {
                return Ok(candidate);
            }
            debug!(url = %candidate, %resolved, "HTTPS candidate redirected to non-HTTPS URL");
        }

        let resolved = probe_live_url(client, url, true)
            .await
            .ok_or(RemoveReason::Unreachable)?;
        if resolved.starts_with("http://") {
            Ok(url.to_string())
        } else {
            warn!(url, %resolved, "HTTP stream redirects across protocol");
            Err(RemoveReason::ProtocolRedirect)
        }
    } else if url.starts_with("https://") {
        let resolved = probe_live_url(client, url, true)
            .await
            .ok_or(RemoveReason::Unreachable)?;
        if resolved.starts_with("https://") {
            Ok(url.to_string())
        } else {
            warn!(url, %resolved, "HTTPS stream redirected to non-HTTPS URL");
            Err(RemoveReason::ProtocolRedirect)
        }
    } else {
        warn!(url, "Stream URL is not HTTP(S)");
        Err(RemoveReason::UnsupportedScheme)
    }
}

fn https_candidate(url: &str) -> Option<String> {
    url.strip_prefix("http://")
        .map(|rest| format!("https://{rest}"))
}

async fn probe_live_url(client: &reqwest::Client, url: &str, log_failures: bool) -> Option<String> {
    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);

    // Try HEAD first. Any non-success response (including connection errors,
    // timeouts, and 4xx) falls through to GET — many Icecast servers don't
    // support HEAD at all and return inconsistent errors.
    let head_ok = match tokio::time::timeout(timeout, client.head(url).send()).await {
        Ok(Ok(resp)) => {
            let s = resp.status();
            if s.is_success() || s.is_redirection() {
                debug!(url, %s, "Stream live (HEAD)");
                return Some(resp.url().to_string());
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
                Some(resp.url().to_string())
            } else {
                if log_failures {
                    warn!(url, %s, "Stream not live");
                } else {
                    debug!(url, %s, "Stream not live");
                }
                None
            }
        }
        Ok(Err(e)) => {
            if log_failures {
                warn!(url, error = %e, "Stream unreachable");
            } else {
                debug!(url, error = %e, "Stream unreachable");
            }
            None
        }
        Err(_) => {
            if log_failures {
                warn!(url, "Stream timed out");
            } else {
                debug!(url, "Stream timed out");
            }
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::https_candidate;

    #[test]
    fn leaves_https_urls_alone() {
        assert_eq!(https_candidate("https://example.org/live.mp3"), None);
    }

    #[test]
    fn upgrades_http_urls_to_https() {
        assert_eq!(
            https_candidate("http://example.org/live.mp3").as_deref(),
            Some("https://example.org/live.mp3")
        );
    }

    #[test]
    fn leaves_non_http_urls_alone() {
        assert_eq!(https_candidate("ftp://example.org/live.mp3"), None);
    }
}

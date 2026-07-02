use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use super::state::{self, StationKey};
use crate::station::Station;

const MAX_CONCURRENT: usize = 50;
const TIMEOUT_SECS: u64 = 10;
/// Consecutive nightly failures before an untrusted station is pruned.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

struct LivenessResult {
    station: Station,
    outcome: Outcome,
}

enum Outcome {
    Trusted,
    Live { upgraded: bool },
    Failed(StreamFailure),
}

pub enum StreamFailure {
    Unreachable,
    GeoBlocked(u16),
    ProtocolRedirect,
    UnsupportedScheme,
}

impl StreamFailure {
    pub fn message(&self) -> &'static str {
        match self {
            StreamFailure::Unreachable => "Stream unreachable during liveness pruning.",
            StreamFailure::GeoBlocked(_) => "Stream refused from the build location.",
            StreamFailure::ProtocolRedirect => "Stream redirects across protocol.",
            StreamFailure::UnsupportedScheme => "Stream URL is not HTTP(S).",
        }
    }

    /// Geo-suspect statuses say nothing about the listener's location and
    /// must never remove a station.
    pub fn is_geo_suspect(&self) -> bool {
        matches!(self, StreamFailure::GeoBlocked(_))
    }

    fn status(&self) -> &'static str {
        match self {
            StreamFailure::Unreachable => "unreachable",
            StreamFailure::GeoBlocked(_) => "geo_blocked",
            StreamFailure::ProtocolRedirect => "protocol_redirect",
            StreamFailure::UnsupportedScheme => "unsupported_scheme",
        }
    }

    /// A scheme failure is a property of the URL itself, not of tonight's
    /// network conditions, so it is pruned without hysteresis.
    fn is_deterministic(&self) -> bool {
        matches!(self, StreamFailure::UnsupportedScheme)
    }
}

pub async fn check(client: &reqwest::Client, stations: Vec<Station>) -> Vec<Station> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let client = client.clone();
    let total = stations.len();
    let store = state::open_from_env();

    let tasks: Vec<_> = stations
        .into_iter()
        .map(|station| {
            let sem = semaphore.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let mut station = station;
                if station.trusted {
                    return LivenessResult {
                        station,
                        outcome: Outcome::Trusted,
                    };
                }

                let _permit = sem.acquire().await.unwrap();
                match validate_imported_stream_url(&client, &station.stream_url).await {
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
                            station,
                            outcome: Outcome::Live { upgraded },
                        }
                    }
                    Err(failure) => LivenessResult {
                        station,
                        outcome: Outcome::Failed(failure),
                    },
                }
            })
        })
        .collect();

    let mut results = Vec::with_capacity(total);
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    if let Some(store) = &store {
        let keys: Vec<StationKey> = results.iter().map(|r| StationKey::of(&r.station)).collect();
        if let Err(e) = store.record_seen(&keys) {
            warn!(error = %e, "Could not record discovery state");
        }
    }

    let mut live = Vec::new();
    let mut pending: Vec<(Station, StreamFailure)> = Vec::new();
    let mut checked = 0usize;
    let mut upgraded = 0usize;
    let mut geo_suspect = 0usize;
    let mut removed_unsupported = 0usize;
    let mut live_keys = Vec::new();
    let mut geo_keys = Vec::new();

    for result in results {
        match result.outcome {
            Outcome::Trusted => live.push(result.station),
            Outcome::Live { upgraded: up } => {
                checked += 1;
                if up {
                    upgraded += 1;
                }
                live_keys.push(StationKey::of(&result.station));
                live.push(result.station);
            }
            Outcome::Failed(StreamFailure::GeoBlocked(status)) => {
                checked += 1;
                geo_suspect += 1;
                debug!(
                    url = %result.station.stream_url,
                    status,
                    "Stream geo-suspect from build location; kept"
                );
                geo_keys.push(StationKey::of(&result.station));
                live.push(result.station);
            }
            Outcome::Failed(failure) if failure.is_deterministic() => {
                checked += 1;
                removed_unsupported += 1;
            }
            Outcome::Failed(failure) => {
                checked += 1;
                pending.push((result.station, failure));
            }
        }
    }

    if let Some(store) = &store {
        if let Err(e) = store.record_live(&live_keys) {
            warn!(error = %e, "Could not record live state");
        }
        if let Err(e) = store.record_geo_blocked(&geo_keys) {
            warn!(error = %e, "Could not record geo-blocked state");
        }
    }

    // Transient failures prune only after MAX_CONSECUTIVE_FAILURES nights in
    // a row. Without a state store there is no memory, so prune immediately
    // as before.
    let mut removed_transient = 0usize;
    let mut failing_kept = 0usize;
    let failure_counts = store.as_ref().and_then(|store| {
        let items: Vec<(StationKey, &str)> = pending
            .iter()
            .map(|(s, f)| (StationKey::of(s), f.status()))
            .collect();
        store
            .record_failures(&items)
            .map_err(|e| warn!(error = %e, "Could not record failure state"))
            .ok()
    });

    for (idx, (station, failure)) in pending.into_iter().enumerate() {
        let count = failure_counts
            .as_ref()
            .map(|c| c[idx])
            .unwrap_or(MAX_CONSECUTIVE_FAILURES);
        if count >= MAX_CONSECUTIVE_FAILURES {
            removed_transient += 1;
            debug!(
                url = %station.stream_url,
                failures = count,
                reason = failure.status(),
                "Pruned after repeated liveness failures"
            );
        } else {
            failing_kept += 1;
            warn!(
                url = %station.stream_url,
                failures = count,
                reason = failure.status(),
                "Liveness failure recorded; station kept"
            );
            live.push(station);
        }
    }

    tracing::info!(
        total,
        checked,
        skipped_trusted = total - checked,
        upgraded,
        geo_suspect,
        failing_kept,
        removed = removed_transient + removed_unsupported,
        removed_transient,
        removed_unsupported_scheme = removed_unsupported,
        live = live.len(),
        "Liveness check complete"
    );
    live
}

pub async fn validate_imported_stream_url(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, StreamFailure> {
    if let Some(candidate) = https_candidate(url) {
        if let Probe::Live(resolved) = probe_live_url(client, &candidate, false).await {
            if resolved.starts_with("https://") {
                return Ok(candidate);
            }
            debug!(url = %candidate, %resolved, "HTTPS candidate redirected to non-HTTPS URL");
        }

        match probe_live_url(client, url, true).await {
            Probe::Live(resolved) => {
                if resolved.starts_with("http://") {
                    Ok(url.to_string())
                } else {
                    warn!(url, %resolved, "HTTP stream redirects across protocol");
                    Err(StreamFailure::ProtocolRedirect)
                }
            }
            Probe::Blocked(status) => Err(StreamFailure::GeoBlocked(status)),
            Probe::Down => Err(StreamFailure::Unreachable),
        }
    } else if url.starts_with("https://") {
        match probe_live_url(client, url, true).await {
            Probe::Live(resolved) => {
                if resolved.starts_with("https://") {
                    Ok(url.to_string())
                } else {
                    warn!(url, %resolved, "HTTPS stream redirected to non-HTTPS URL");
                    Err(StreamFailure::ProtocolRedirect)
                }
            }
            Probe::Blocked(status) => Err(StreamFailure::GeoBlocked(status)),
            Probe::Down => Err(StreamFailure::Unreachable),
        }
    } else {
        warn!(url, "Stream URL is not HTTP(S)");
        Err(StreamFailure::UnsupportedScheme)
    }
}

fn https_candidate(url: &str) -> Option<String> {
    url.strip_prefix("http://")
        .map(|rest| format!("https://{rest}"))
}

enum Probe {
    Live(String),
    /// The server answered with a status that commonly means the request was
    /// refused because of where it came from (403 Forbidden, 451 Unavailable
    /// For Legal Reasons), not because the stream is dead.
    Blocked(u16),
    Down,
}

fn blocked_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 403 | 451)
}

async fn probe_live_url(client: &reqwest::Client, url: &str, log_failures: bool) -> Probe {
    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);

    // Try HEAD first. Any non-success response (including connection errors,
    // timeouts, and 4xx) falls through to GET — many Icecast servers don't
    // support HEAD at all and return inconsistent errors.
    if let Ok(Ok(resp)) = tokio::time::timeout(timeout, client.head(url).send()).await {
        let s = resp.status();
        if s.is_success() || s.is_redirection() {
            debug!(url, %s, "Stream live (HEAD)");
            return Probe::Live(resp.url().to_string());
        }
    }

    // GET fallback: we only need the response status, not the body.
    // reqwest won't download the body until we call .bytes()/.text(), so this
    // is cheap even for infinite audio streams.
    match tokio::time::timeout(timeout, client.get(url).send()).await {
        Ok(Ok(resp)) => {
            let s = resp.status();
            if s.is_success() || s.is_redirection() {
                debug!(url, %s, "Stream live (GET)");
                Probe::Live(resp.url().to_string())
            } else if blocked_status(s) {
                if log_failures {
                    warn!(url, %s, "Stream refused; possibly geo-blocked");
                } else {
                    debug!(url, %s, "Stream refused; possibly geo-blocked");
                }
                Probe::Blocked(s.as_u16())
            } else {
                if log_failures {
                    warn!(url, %s, "Stream not live");
                } else {
                    debug!(url, %s, "Stream not live");
                }
                Probe::Down
            }
        }
        Ok(Err(e)) => {
            if log_failures {
                warn!(url, error = %e, "Stream unreachable");
            } else {
                debug!(url, error = %e, "Stream unreachable");
            }
            Probe::Down
        }
        Err(_) => {
            if log_failures {
                warn!(url, "Stream timed out");
            } else {
                debug!(url, "Stream timed out");
            }
            Probe::Down
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StreamFailure, blocked_status, https_candidate};

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

    #[test]
    fn geo_suspect_statuses() {
        assert!(blocked_status(reqwest::StatusCode::FORBIDDEN));
        assert!(blocked_status(
            reqwest::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
        ));
        assert!(!blocked_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!blocked_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn only_scheme_failures_are_deterministic() {
        assert!(StreamFailure::UnsupportedScheme.is_deterministic());
        assert!(!StreamFailure::Unreachable.is_deterministic());
        assert!(!StreamFailure::ProtocolRedirect.is_deterministic());
        assert!(StreamFailure::GeoBlocked(403).is_geo_suspect());
        assert!(!StreamFailure::Unreachable.is_geo_suspect());
    }
}

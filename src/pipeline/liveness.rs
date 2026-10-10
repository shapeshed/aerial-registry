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
    Gone(u16),
    ConnectionError,
    Inconclusive(String),
    UnsupportedScheme,
}

impl StreamFailure {
    pub fn message(&self) -> &'static str {
        match self {
            StreamFailure::Gone(status) if *status == 410 => "Stream URL returned 410 Gone.",
            StreamFailure::Gone(_) => "Stream URL returned 404 Not Found.",
            StreamFailure::ConnectionError => "Stream unreachable during liveness pruning.",
            StreamFailure::Inconclusive(_) => {
                "Stream could not be confirmed from the build location."
            }
            StreamFailure::UnsupportedScheme => "Stream URL is not HTTP(S).",
        }
    }

    /// Inconclusive statuses say nothing reliable about listeners elsewhere and
    /// must never remove a station.
    pub fn is_inconclusive(&self) -> bool {
        matches!(self, StreamFailure::Inconclusive(_))
    }

    fn status(&self) -> &'static str {
        match self {
            StreamFailure::Gone(_) => "gone",
            StreamFailure::ConnectionError => "connection_error",
            StreamFailure::Inconclusive(_) => "inconclusive",
            StreamFailure::UnsupportedScheme => "unsupported_scheme",
        }
    }

    /// These failures are properties of the URL itself, not of tonight's
    /// network conditions, so they are pruned without hysteresis.
    /// `ConnectionError` is deliberately *not* here: from a single build
    /// location it can be a transient blip or a geo-block that drops the
    /// connection, so it goes through the three-strike hysteresis instead.
    fn is_deterministic(&self) -> bool {
        matches!(
            self,
            StreamFailure::Gone(_) | StreamFailure::UnsupportedScheme
        )
    }
}

pub async fn check(client: &crate::http::Client, stations: Vec<Station>) -> Vec<Station> {
    // Local builds behind a proxy (or quick iterations) can skip the probes
    // entirely; nothing is pruned and no failure state is recorded.
    if std::env::var("AERIAL_SKIP_LIVENESS").is_ok_and(|v| !v.is_empty() && v != "0") {
        info!(
            total = stations.len(),
            "Liveness checks skipped (AERIAL_SKIP_LIVENESS)"
        );
        return stations;
    }

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
    let mut inconclusive = 0usize;
    let mut removed_deterministic = 0usize;
    let mut live_keys = Vec::new();
    let mut inconclusive_keys = Vec::new();

    for result in results {
        let LivenessResult { station, outcome } = result;

        // Curated stations are hand-picked and reviewed, so the nightly build
        // never auto-prunes them: a stream that fails from the build location
        // is kept, and genuinely dead entries are handled deliberately via the
        // separate `prune-curated` command.
        if station.provider == "curated" {
            if let Outcome::Failed(failure) = &outcome {
                checked += 1;
                debug!(
                    url = %station.stream_url,
                    reason = failure.status(),
                    "Curated stream failed liveness; kept"
                );
                live.push(station);
                continue;
            }
        }

        match outcome {
            Outcome::Trusted => live.push(station),
            Outcome::Live { upgraded: up } => {
                checked += 1;
                if up {
                    upgraded += 1;
                }
                live_keys.push(StationKey::of(&station));
                live.push(station);
            }
            Outcome::Failed(StreamFailure::Inconclusive(reason)) => {
                checked += 1;
                inconclusive += 1;
                debug!(
                    url = %station.stream_url,
                    %reason,
                    "Stream inconclusive from build location; kept"
                );
                inconclusive_keys.push(StationKey::of(&station));
                live.push(station);
            }
            Outcome::Failed(failure) if failure.is_deterministic() => {
                checked += 1;
                removed_deterministic += 1;
                warn!(
                    url = %station.stream_url,
                    reason = failure.status(),
                    "Removed station after deterministic liveness failure"
                );
            }
            Outcome::Failed(failure) => {
                checked += 1;
                pending.push((station, failure));
            }
        }
    }

    if let Some(store) = &store {
        if let Err(e) = store.record_live(&live_keys) {
            warn!(error = %e, "Could not record live state");
        }
        if let Err(e) = store.record_geo_blocked(&inconclusive_keys) {
            warn!(error = %e, "Could not record inconclusive liveness state");
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
        inconclusive,
        failing_kept,
        removed = removed_transient + removed_deterministic,
        removed_transient,
        removed_deterministic,
        live = live.len(),
        "Liveness check complete"
    );
    live
}

pub async fn validate_imported_stream_url(
    client: &crate::http::Client,
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
                    warn!(url, %resolved, "HTTP stream redirects across protocol; keeping original URL");
                    Ok(url.to_string())
                }
            }
            Probe::Inconclusive(reason) => Err(StreamFailure::Inconclusive(reason)),
            Probe::Gone(status) => Err(StreamFailure::Gone(status)),
            Probe::ConnectionError => Err(StreamFailure::ConnectionError),
        }
    } else if url.starts_with("https://") {
        match probe_live_url(client, url, true).await {
            Probe::Live(resolved) => {
                if resolved.starts_with("https://") {
                    Ok(url.to_string())
                } else {
                    warn!(url, %resolved, "HTTPS stream redirected to non-HTTPS URL; keeping original URL");
                    Ok(url.to_string())
                }
            }
            Probe::Inconclusive(reason) => Err(StreamFailure::Inconclusive(reason)),
            Probe::Gone(status) => Err(StreamFailure::Gone(status)),
            Probe::ConnectionError => Err(StreamFailure::ConnectionError),
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
    /// The server answered or timed out in a way that does not prove the stream
    /// is dead for listeners elsewhere.
    Inconclusive(String),
    Gone(u16),
    ConnectionError,
}

fn gone_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
    )
}

async fn probe_live_url(client: &crate::http::Client, url: &str, log_failures: bool) -> Probe {
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
            } else if gone_status(s) {
                if log_failures {
                    warn!(url, %s, "Stream gone");
                } else {
                    debug!(url, %s, "Stream gone");
                }
                Probe::Gone(s.as_u16())
            } else {
                if log_failures {
                    warn!(url, %s, "Stream status inconclusive; kept");
                } else {
                    debug!(url, %s, "Stream status inconclusive; kept");
                }
                Probe::Inconclusive(format!("http_status_{}", s.as_u16()))
            }
        }
        Ok(Err(e)) => {
            if log_failures {
                warn!(url, error = %e, "Stream unreachable");
            } else {
                debug!(url, error = %e, "Stream unreachable");
            }
            Probe::ConnectionError
        }
        Err(_) => {
            if log_failures {
                warn!(url, "Stream timed out; kept");
            } else {
                debug!(url, "Stream timed out; kept");
            }
            Probe::Inconclusive("timeout".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StreamFailure, gone_status, https_candidate};

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
    fn only_gone_statuses_are_removed() {
        assert!(gone_status(reqwest::StatusCode::NOT_FOUND));
        assert!(gone_status(reqwest::StatusCode::GONE));
        assert!(!gone_status(reqwest::StatusCode::FORBIDDEN));
        assert!(!gone_status(
            reqwest::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
        ));
        assert!(!gone_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn deterministic_failures_are_classified() {
        assert!(StreamFailure::Gone(404).is_deterministic());
        assert!(StreamFailure::UnsupportedScheme.is_deterministic());
        // A connection error can be transient or a geo-block, so it must go
        // through hysteresis rather than being pruned on the first night.
        assert!(!StreamFailure::ConnectionError.is_deterministic());
        assert!(StreamFailure::Inconclusive("timeout".into()).is_inconclusive());
        assert!(!StreamFailure::ConnectionError.is_inconclusive());
    }
}

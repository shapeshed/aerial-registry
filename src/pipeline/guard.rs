use std::collections::{HashMap, HashSet};
use std::io::Read;

use flate2::read::GzDecoder;
use tracing::{info, warn};

use crate::station::Station;

/// One provider the guard stepped in for.
pub struct Intervention {
    pub provider: String,
    pub previous: usize,
    pub discovered: usize,
    pub carried: usize,
}

/// Load the previously shipped registry for guard comparison and the diff
/// report, from a local file — nothing is hosted publicly for this to pull
/// from over the network anymore.
///
/// Set `AERIAL_PREVIOUS_REGISTRY_PATH` to the path of a `registry.json` or
/// `registry.json.gz` to compare against — typically a local copy of the
/// app's currently-shipped `app/src/main/registry/registry.json`, i.e. the
/// last human-approved state. Unset (the common case for an ad hoc local
/// run) means no previous state is available, so the guard and diff report
/// are both skipped.
pub fn load_from_env() -> Option<Vec<Station>> {
    load_from_path(std::env::var("AERIAL_PREVIOUS_REGISTRY_PATH").ok())
}

fn load_from_path(path: Option<String>) -> Option<Vec<Station>> {
    let path = match path {
        Some(v) if !v.is_empty() => v,
        _ => {
            info!("No AERIAL_PREVIOUS_REGISTRY_PATH set; previous-registry guard disabled");
            return None;
        }
    };

    match load_registry(&path) {
        Ok(p) => Some(p),
        Err(e) => {
            warn!(path, error = %e, "Could not load previous registry; guard skipped");
            None
        }
    }
}

/// Guards the published registry against transient provider failures.
///
/// A provider API that times out or returns a malformed response during one
/// nightly run would otherwise silently remove every one of its stations from
/// the published registry. Compare per-provider station counts against the
/// previously published registry and, where a provider lost more than half of
/// its stations, carry yesterday's entries forward instead of publishing the
/// hole.
pub fn apply(
    stations: Vec<Station>,
    previous: Option<&[Station]>,
) -> (Vec<Station>, Vec<Intervention>) {
    match previous {
        Some(previous) => merge(stations, previous),
        None => (stations, Vec::new()),
    }
}

fn load_registry(path: &str) -> anyhow::Result<Vec<Station>> {
    let bytes = std::fs::read(path)?;

    // Accept either a plain registry.json or a gzipped registry.json.gz —
    // sniff the magic bytes rather than trusting the file extension.
    let json = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(bytes.as_slice()).read_to_end(&mut out)?;
        out
    } else {
        bytes
    };

    Ok(serde_json::from_slice(&json)?)
}

fn merge(current: Vec<Station>, previous: &[Station]) -> (Vec<Station>, Vec<Intervention>) {
    let current_counts = count_by_provider(&current);
    let previous_counts = count_by_provider(previous);

    let failed: Vec<&String> = previous_counts
        .keys()
        .filter(|provider| {
            let prev = previous_counts[*provider];
            let now = current_counts.get(*provider).copied().unwrap_or(0);
            lost_too_many(prev, now)
        })
        .collect();

    if failed.is_empty() {
        info!("Previous-registry guard passed; no provider anomalies");
        return (current, Vec::new());
    }

    // Drop the failed providers' partial output and carry forward yesterday's
    // entries, skipping any whose stream URL another provider now owns.
    let mut out: Vec<Station> = current
        .into_iter()
        .filter(|s| !failed.contains(&&s.provider))
        .collect();
    let seen: HashSet<String> = out
        .iter()
        .map(|s| super::dedup::normalise_url(&s.stream_url))
        .collect();

    let mut interventions = Vec::new();
    for provider in &failed {
        let carried: Vec<Station> = previous
            .iter()
            .filter(|s| {
                &&s.provider == provider
                    && !seen.contains(&super::dedup::normalise_url(&s.stream_url))
            })
            .cloned()
            .collect();
        let intervention = Intervention {
            provider: (*provider).clone(),
            previous: previous_counts[*provider],
            discovered: current_counts.get(*provider).copied().unwrap_or(0),
            carried: carried.len(),
        };
        warn!(
            provider = intervention.provider.as_str(),
            previous = intervention.previous,
            discovered = intervention.discovered,
            carried = intervention.carried,
            "Provider lost more than half its stations; carrying forward previous entries"
        );
        interventions.push(intervention);
        out.extend(carried);
    }

    (out, interventions)
}

fn count_by_provider(stations: &[Station]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for s in stations {
        *counts.entry(s.provider.clone()).or_insert(0) += 1;
    }
    counts
}

fn lost_too_many(previous: usize, current: usize) -> bool {
    current * 2 < previous
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(provider: &str, url: &str) -> Station {
        Station {
            name: url.to_string(),
            stream_url: url.to_string(),
            logo_url: None,
            country: None,
            country_code: None,
            tags: vec![],
            description: None,
            provider: provider.to_string(),
            provider_id: None,
            trusted: false,
        }
    }

    #[test]
    fn lost_too_many_triggers_on_majority_drop() {
        assert!(lost_too_many(9, 0));
        assert!(lost_too_many(9, 4));
        assert!(lost_too_many(1, 0));
        assert!(!lost_too_many(9, 5));
        assert!(!lost_too_many(2, 1));
        assert!(!lost_too_many(0, 0));
    }

    #[test]
    fn healthy_providers_pass_through_unchanged() {
        let current = vec![station("bbc", "a"), station("bbc", "b")];
        let previous = vec![station("bbc", "a"), station("bbc", "b")];
        let (out, interventions) = merge(current, &previous);
        assert_eq!(out.len(), 2);
        assert!(interventions.is_empty());
    }

    #[test]
    fn failed_provider_entries_are_carried_forward() {
        let current = vec![station("bbc", "a")];
        let previous = vec![
            station("bbc", "a"),
            station("wireless", "w1"),
            station("wireless", "w2"),
        ];
        let (out, interventions) = merge(current, &previous);
        assert_eq!(out.len(), 3);
        assert_eq!(out.iter().filter(|s| s.provider == "wireless").count(), 2);
        assert_eq!(interventions.len(), 1);
        assert_eq!(interventions[0].provider, "wireless");
        assert_eq!(interventions[0].carried, 2);
    }

    #[test]
    fn partial_output_is_replaced_not_duplicated() {
        // Provider found 1 of its previous 4 stations: drop the partial set
        // and carry all 4 previous entries forward.
        let current = vec![station("rinse", "r1")];
        let previous = vec![
            station("rinse", "r1"),
            station("rinse", "r2"),
            station("rinse", "r3"),
            station("rinse", "r4"),
        ];
        let (out, _) = merge(current, &previous);
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn carried_entries_skip_urls_owned_elsewhere() {
        let current = vec![station("curated", "shared")];
        let previous = vec![station("wireless", "shared"), station("wireless", "w2")];
        let (out, interventions) = merge(current, &previous);
        assert_eq!(out.len(), 2);
        assert_eq!(out.iter().filter(|s| s.provider == "wireless").count(), 1);
        assert_eq!(interventions[0].carried, 1);
    }

    #[test]
    fn new_providers_are_not_penalised() {
        // A provider present today but absent yesterday must not be touched.
        let current = vec![station("sbs", "s1")];
        let previous = vec![];
        let (out, interventions) = merge(current, &previous);
        assert_eq!(out.len(), 1);
        assert!(interventions.is_empty());
    }

    #[test]
    fn apply_without_previous_is_a_passthrough() {
        let current = vec![station("bbc", "a")];
        let (out, interventions) = apply(current, None);
        assert_eq!(out.len(), 1);
        assert!(interventions.is_empty());
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "aerial-registry-guard-test-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn load_registry_reads_plain_json() {
        let path = temp_path("plain");
        let stations = vec![station("bbc", "a")];
        std::fs::write(&path, serde_json::to_vec(&stations).unwrap()).unwrap();

        let loaded = load_registry(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].provider, "bbc");
    }

    #[test]
    fn load_registry_sniffs_and_decompresses_gzip() {
        use std::io::Write as _;

        let path = temp_path("gz");
        let stations = vec![station("wireless", "w1")];
        let json = serde_json::to_vec(&stations).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&json).unwrap();
        let gz_bytes = encoder.finish().unwrap();
        std::fs::write(&path, gz_bytes).unwrap();

        let loaded = load_registry(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].provider, "wireless");
    }

    #[test]
    fn load_registry_errors_on_missing_file() {
        assert!(load_registry("/nonexistent/path/registry.json").is_err());
    }

    // `load_from_path` (not `load_from_env`) so these don't mutate real
    // process env vars — tests run concurrently within one process, and
    // that would race against each other.
    #[test]
    fn load_from_path_skips_when_none() {
        assert!(load_from_path(None).is_none());
        assert!(load_from_path(Some(String::new())).is_none());
    }

    #[test]
    fn load_from_path_reads_the_given_path() {
        let path = temp_path("env-set");
        let stations = vec![station("bbc", "a")];
        std::fs::write(&path, serde_json::to_vec(&stations).unwrap()).unwrap();

        let loaded = load_from_path(Some(path.to_str().unwrap().to_string()));
        std::fs::remove_file(&path).unwrap();

        assert_eq!(loaded.map(|s| s.len()), Some(1));
    }
}

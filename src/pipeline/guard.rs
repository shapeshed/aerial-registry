use std::collections::{HashMap, HashSet};
use std::io::Read;

use flate2::read::GzDecoder;
use tracing::{info, warn};

use crate::station::Station;

const DEFAULT_PREVIOUS_URL: &str = "https://aerial.shapeshed.com/registry.json.gz";

/// One provider the guard stepped in for.
pub struct Intervention {
    pub provider: String,
    pub previous: usize,
    pub discovered: usize,
    pub carried: usize,
}

/// Fetch the previously published registry for guard comparison and the
/// nightly diff report.
///
/// Set `AERIAL_PREVIOUS_REGISTRY_URL` to override the source, or set it to an
/// empty string to disable the guard (and the diff report with it).
pub async fn fetch(client: &reqwest::Client) -> Option<Vec<Station>> {
    let url = match std::env::var("AERIAL_PREVIOUS_REGISTRY_URL") {
        Ok(v) if v.is_empty() => {
            info!("Previous-registry guard disabled");
            return None;
        }
        Ok(v) => v,
        Err(_) => DEFAULT_PREVIOUS_URL.to_string(),
    };

    match fetch_registry(client, &url).await {
        Ok(p) => Some(p),
        Err(e) => {
            warn!(url, error = %e, "Could not fetch previous registry; guard skipped");
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

async fn fetch_registry(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<Station>> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let bytes = resp.bytes().await?;

    // S3 stores the registry gzipped with `content-encoding: gzip`, but the
    // client has no automatic decompression, so sniff the magic bytes.
    let json = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(bytes.as_ref()).read_to_end(&mut out)?;
        out
    } else {
        bytes.to_vec()
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
}

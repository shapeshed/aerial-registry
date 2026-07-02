use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;

use tracing::{info, warn};

use super::guard::Intervention;
use crate::station::Station;

const ANOMALIES_PATH: &str = "anomalies.md";

/// Summarise what changed against the previously published registry.
///
/// The full diff is appended to `$GITHUB_STEP_SUMMARY` when running in
/// Actions. When the guard intervened, the intervention table is also written
/// to `anomalies.md`, which the nightly workflow turns into a GitHub issue —
/// anomalies are the only thing that should page a human.
pub fn write(previous: Option<&[Station]>, current: &[Station], interventions: &[Intervention]) {
    let Some(previous) = previous else {
        info!("No previous registry available; diff report skipped");
        return;
    };

    let diff = diff(previous, current);
    info!(
        previous = previous.len(),
        current = current.len(),
        added = diff.added,
        removed = diff.removed,
        renamed = diff.renamed.len(),
        guard_interventions = interventions.len(),
        "Registry diff"
    );

    let summary = render(previous.len(), current.len(), &diff, interventions);
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY")
        && let Err(e) = append(&path, &summary)
    {
        warn!(path, error = %e, "Could not write step summary");
    }

    if !interventions.is_empty()
        && let Err(e) = std::fs::write(ANOMALIES_PATH, render_anomalies(interventions))
    {
        warn!(error = %e, "Could not write anomalies file");
    }
}

struct Diff {
    added: usize,
    removed: usize,
    renamed: Vec<(String, String, String)>, // provider, old name, new name
    providers: BTreeMap<String, ProviderDiff>,
}

#[derive(Default)]
struct ProviderDiff {
    previous: usize,
    current: usize,
    added: usize,
    removed: usize,
}

fn key(station: &Station) -> (String, String) {
    let id = match station.provider_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => station.stream_url.clone(),
    };
    (station.provider.clone(), id)
}

fn diff(previous: &[Station], current: &[Station]) -> Diff {
    let prev_names: HashMap<(String, String), &str> =
        previous.iter().map(|s| (key(s), s.name.as_str())).collect();
    let curr_names: HashMap<(String, String), &str> =
        current.iter().map(|s| (key(s), s.name.as_str())).collect();

    let mut providers: BTreeMap<String, ProviderDiff> = BTreeMap::new();
    for s in previous {
        providers.entry(s.provider.clone()).or_default().previous += 1;
    }
    for s in current {
        providers.entry(s.provider.clone()).or_default().current += 1;
    }

    let mut added = 0;
    let mut removed = 0;
    let mut renamed = Vec::new();

    for (k, name) in &curr_names {
        match prev_names.get(k) {
            None => {
                added += 1;
                providers.entry(k.0.clone()).or_default().added += 1;
            }
            Some(old) if old != name => {
                renamed.push((k.0.clone(), old.to_string(), name.to_string()));
            }
            Some(_) => {}
        }
    }
    for k in prev_names.keys() {
        if !curr_names.contains_key(k) {
            removed += 1;
            providers.entry(k.0.clone()).or_default().removed += 1;
        }
    }
    renamed.sort();

    Diff {
        added,
        removed,
        renamed,
        providers,
    }
}

fn render(
    previous_total: usize,
    current_total: usize,
    diff: &Diff,
    interventions: &[Intervention],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## Registry build report\n");
    let _ = writeln!(
        out,
        "**{current_total}** stations published (previous {previous_total}, +{} added, \u{2212}{} removed, {} renamed)\n",
        diff.added,
        diff.removed,
        diff.renamed.len()
    );

    if !interventions.is_empty() {
        let _ = writeln!(out, "### \u{26a0}\u{fe0f} Guard interventions\n");
        out.push_str(&intervention_table(interventions));
        out.push('\n');
    }

    let _ = writeln!(out, "### Providers\n");
    let _ = writeln!(out, "| Provider | Previous | Current | Added | Removed |");
    let _ = writeln!(out, "|---|---:|---:|---:|---:|");
    for (provider, p) in &diff.providers {
        let _ = writeln!(
            out,
            "| {provider} | {} | {} | {} | {} |",
            p.previous, p.current, p.added, p.removed
        );
    }

    if !diff.renamed.is_empty() {
        let _ = writeln!(out, "\n### Renamed\n");
        let _ = writeln!(out, "| Provider | Was | Now |");
        let _ = writeln!(out, "|---|---|---|");
        for (provider, old, new) in &diff.renamed {
            let _ = writeln!(out, "| {provider} | {old} | {new} |");
        }
    }

    out
}

fn render_anomalies(interventions: &[Intervention]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "The nightly build carried forward previously published entries for \
         providers that lost more than half of their stations in one run:\n"
    );
    out.push_str(&intervention_table(interventions));
    let _ = writeln!(
        out,
        "\nThis usually means a provider API failed or changed shape. The \
         published registry is protected, but discovery for these providers \
         needs a look."
    );
    out
}

fn intervention_table(interventions: &[Intervention]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "| Provider | Previous | Discovered | Carried |");
    let _ = writeln!(out, "|---|---:|---:|---:|");
    for i in interventions {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            i.provider, i.previous, i.discovered, i.carried
        );
    }
    out
}

fn append(path: &str, content: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{content}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(provider: &str, id: &str, name: &str) -> Station {
        Station {
            name: name.to_string(),
            stream_url: format!("https://example.com/{id}"),
            logo_url: None,
            country: None,
            country_code: None,
            tags: vec![],
            description: None,
            provider: provider.to_string(),
            provider_id: Some(id.to_string()),
            trusted: false,
        }
    }

    #[test]
    fn diff_counts_added_removed_renamed() {
        let previous = vec![
            station("bbc", "one", "BBC One"),
            station("bbc", "two", "BBC Two"),
            station("dr", "p1", "DR P1"),
        ];
        let current = vec![
            station("bbc", "one", "BBC Radio One"),
            station("dr", "p1", "DR P1"),
            station("dr", "p2", "DR P2"),
        ];
        let d = diff(&previous, &current);
        assert_eq!(d.added, 1);
        assert_eq!(d.removed, 1);
        assert_eq!(
            d.renamed,
            vec![(
                "bbc".to_string(),
                "BBC One".to_string(),
                "BBC Radio One".to_string()
            )]
        );
        assert_eq!(d.providers["bbc"].previous, 2);
        assert_eq!(d.providers["bbc"].current, 1);
        assert_eq!(d.providers["dr"].added, 1);
    }

    #[test]
    fn same_provider_id_across_providers_does_not_collide() {
        // NRK and DR both use p1/p2/p3.
        let previous = vec![station("dr", "p1", "DR P1"), station("nrk", "p1", "NRK P1")];
        let current = vec![station("dr", "p1", "DR P1")];
        let d = diff(&previous, &current);
        assert_eq!(d.removed, 1);
        assert_eq!(d.providers["nrk"].removed, 1);
        assert_eq!(d.providers["dr"].removed, 0);
    }

    #[test]
    fn render_includes_intervention_table_only_when_present() {
        let d = diff(&[], &[]);
        let quiet = render(0, 0, &d, &[]);
        assert!(!quiet.contains("Guard interventions"));

        let noisy = render(
            10,
            10,
            &d,
            &[Intervention {
                provider: "wireless".to_string(),
                previous: 9,
                discovered: 0,
                carried: 9,
            }],
        );
        assert!(noisy.contains("Guard interventions"));
        assert!(noisy.contains("| wireless | 9 | 0 | 9 |"));
    }
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::ai;
use crate::station::Station;

const OVERLAY_PATH: &str = "enrichment.toml";

/// One reviewed enrichment result, keyed by the station's cross-run identity.
///
/// `enrichment.toml` is committed: the nightly build applies it with no model
/// or network dependency, and the weekly `enrich-overlay` job PRs updates for
/// stations that are new or whose provider-supplied fields changed
/// (`source_hash`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub provider: String,
    pub provider_id: String,
    pub source_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reject: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OverlayFile {
    #[serde(default, rename = "station")]
    stations: Vec<Entry>,
}

/// Apply the committed overlay to the discovered stations. Runs in the
/// nightly build after enrich and before liveness; deterministic and offline.
pub fn apply(stations: Vec<Station>) -> Vec<Station> {
    let entries = load();
    if entries.is_empty() {
        info!("No enrichment overlay; skipping");
        return stations;
    }
    let by_key: HashMap<(String, String), &Entry> = entries
        .iter()
        .map(|e| ((e.provider.clone(), e.provider_id.clone()), e))
        .collect();

    let total = stations.len();
    let mut applied = 0usize;
    let mut rejected = 0usize;
    let mut stale = 0usize;

    let out: Vec<Station> = stations
        .into_iter()
        .filter_map(|mut station| {
            let Some(entry) = by_key.get(&key(&station)) else {
                return Some(station);
            };
            if entry.source_hash != source_hash(&station) {
                // Provider data moved on since this entry was reviewed; still
                // apply it (better than raw), the weekly job will refresh it.
                stale += 1;
            }
            if entry.reject {
                rejected += 1;
                return None;
            }
            if let Some(name) = &entry.name {
                station.name = name.clone();
            }
            if let Some(tags) = &entry.tags {
                station.tags = tags.clone();
            }
            if let Some(description) = &entry.description {
                station.description = Some(description.clone());
            }
            applied += 1;
            Some(station)
        })
        .collect();

    info!(
        total,
        entries = entries.len(),
        applied,
        rejected,
        stale,
        "Enrichment overlay applied"
    );
    out
}

/// Weekly job: assess stations that are new or whose source fields changed,
/// then rewrite `enrichment.toml`. The workflow turns the diff into a PR.
pub async fn build(client: &reqwest::Client) -> anyhow::Result<()> {
    let Some(config) = ai::config_from_env() else {
        anyhow::bail!("AERIAL_AI_URL and AERIAL_AI_MODEL must be set for enrich-overlay");
    };

    let all = crate::providers::discover_all(client).await;
    let deduped = super::dedup::dedup(all);
    let stations = super::enrich::enrich(client, deduped).await;

    let mut entries: HashMap<(String, String), Entry> = load()
        .into_iter()
        .map(|e| ((e.provider.clone(), e.provider_id.clone()), e))
        .collect();

    // Drop entries for stations that no longer exist upstream.
    let live_keys: std::collections::HashSet<(String, String)> = stations.iter().map(key).collect();
    let before = entries.len();
    entries.retain(|k, _| live_keys.contains(k));
    let removed = before - entries.len();

    let pending: Vec<&Station> = stations
        .iter()
        .filter(|s| {
            entries
                .get(&key(s))
                .map(|e| e.source_hash != source_hash(s))
                .unwrap_or(true)
        })
        .collect();
    let limit = config.limit.unwrap_or(pending.len());
    let capped = pending.len().min(limit);
    if pending.len() > capped {
        info!(
            pending = pending.len(),
            capped, "AERIAL_AI_LIMIT reached; remaining stations left for the next run"
        );
    }
    info!(
        stations = stations.len(),
        unchanged = stations.len() - pending.len(),
        assessing = capped,
        removed_entries = removed,
        "Enrichment overlay delta"
    );

    // Model calls get their own client: the pipeline client's 15s timeout
    // would cancel local LLM generations mid-token.
    let ai_client = crate::http::build_ai_client()?;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(config.concurrency));
    let tasks = pending[..capped].iter().map(|station| {
        let client = ai_client.clone();
        let config = config.clone();
        let semaphore = semaphore.clone();
        async move {
            let _permit = semaphore.acquire().await.expect("semaphore is open");
            let assessment = ai::assess(&client, &config, station).await;
            (*station, assessment)
        }
    });
    let results = futures::future::join_all(tasks).await;

    let mut assessed = 0usize;
    for (station, assessment) in results {
        match assessment {
            Ok(Some(assessment)) => {
                let entry = entry_from(station, &assessment);
                log_assessment(station, &assessment, &entry);
                if let Err(e) = write_audit(station, &assessment, &entry) {
                    warn!(error = %e, "Failed to write AI audit record");
                }
                entries.insert(key(station), entry);
                assessed += 1;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(
                    provider = %station.provider,
                    name = %station.name,
                    error = %e,
                    "AI assessment failed"
                );
            }
        }
    }

    drop_colliding_names(&stations, &mut entries);

    let mut sorted: Vec<Entry> = entries.into_values().collect();
    sorted.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then(a.provider_id.cmp(&b.provider_id))
    });
    let count = sorted.len();
    save(sorted)?;
    info!(assessed, entries = count, "Enrichment overlay written");
    Ok(())
}

/// Drop name overrides that would leave two stations of one provider with
/// the same display name (run 3: two France Musique variants both renamed
/// to plain "France Musique", colliding with the real one).
fn drop_colliding_names(stations: &[Station], entries: &mut HashMap<(String, String), Entry>) {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for station in stations {
        let display = entries
            .get(&key(station))
            .and_then(|e| e.name.clone())
            .unwrap_or_else(|| station.name.clone());
        *counts
            .entry((station.provider.clone(), display))
            .or_insert(0) += 1;
    }
    for station in stations {
        let Some(entry) = entries.get_mut(&key(station)) else {
            continue;
        };
        let Some(name) = entry.name.clone() else {
            continue;
        };
        if counts[&(station.provider.clone(), name.clone())] > 1 {
            warn!(
                provider = %station.provider,
                old = %station.name,
                new = %name,
                "Name override collides with another station; dropped"
            );
            entry.name = None;
        }
    }
}

/// Log old → new for every assessment so a local run reads as a review
/// stream, whatever the log level of the surrounding pipeline noise.
fn log_assessment(station: &Station, assessment: &ai::AiAssessment, entry: &Entry) {
    info!(
        provider = %station.provider,
        provider_id = %entry.provider_id,
        old_name = %station.name,
        new_name = entry.name.as_deref().unwrap_or(&station.name),
        old_tags = ?station.tags,
        new_tags = ?entry.tags.as_deref().unwrap_or(&station.tags),
        old_description = station.description.as_deref().unwrap_or(""),
        new_description = entry
            .description
            .as_deref()
            .or(station.description.as_deref())
            .unwrap_or(""),
        reject = entry.reject,
        confidence = assessment.confidence,
        reason = %assessment.reason,
        "AI assessment"
    );
}

#[derive(Serialize)]
struct AuditRecord<'a> {
    provider: &'a str,
    provider_id: &'a str,
    old_name: &'a str,
    new_name: &'a str,
    old_tags: &'a [String],
    new_tags: &'a [String],
    old_description: Option<&'a str>,
    new_description: Option<&'a str>,
    accepted: bool,
    reject: bool,
    confidence: f32,
    risks: &'a [String],
    reason: &'a str,
}

/// Append a JSONL audit record when `AERIAL_AI_AUDIT` names a file — the
/// review artifact for comparing models over the same sample.
fn write_audit(
    station: &Station,
    assessment: &ai::AiAssessment,
    entry: &Entry,
) -> anyhow::Result<()> {
    let Ok(path) = std::env::var("AERIAL_AI_AUDIT") else {
        return Ok(());
    };
    if path.is_empty() {
        return Ok(());
    }
    let record = AuditRecord {
        provider: &station.provider,
        provider_id: &entry.provider_id,
        old_name: &station.name,
        new_name: entry.name.as_deref().unwrap_or(&station.name),
        old_tags: &station.tags,
        new_tags: entry.tags.as_deref().unwrap_or(&station.tags),
        old_description: station.description.as_deref(),
        new_description: entry
            .description
            .as_deref()
            .or(station.description.as_deref()),
        accepted: assessment.accept,
        reject: entry.reject,
        confidence: assessment.confidence,
        risks: &assessment.risks,
        reason: &assessment.reason,
    };
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(&record)?)?;
    Ok(())
}

fn entry_from(station: &Station, assessment: &ai::AiAssessment) -> Entry {
    let (provider, provider_id) = key(station);
    let hash = source_hash(station);

    // Rejection only ever drops aggregator/curated records; a trusted
    // broadcaster feed the model dislikes is a model problem, not a station
    // problem.
    if !assessment.accept && !station.trusted {
        info!(
            provider,
            name = %station.name,
            reason = %assessment.reason,
            "AI rejected station; overlay will drop it"
        );
        return Entry {
            provider,
            provider_id,
            source_hash: hash,
            name: None,
            tags: None,
            description: None,
            reject: true,
        };
    }

    if assessment.confidence < ai::APPLY_CONFIDENCE {
        // Not confident enough to override anything: record the hash so the
        // station is not re-assessed every week, but change nothing.
        return Entry {
            provider,
            provider_id,
            source_hash: hash,
            name: None,
            tags: None,
            description: None,
            reject: false,
        };
    }

    let mut name = ai::canonicalize_name(&assessment.canonical_name);
    // Feed-distinguishing suffixes must survive the rename or the two feeds
    // of one station end up with identical display names.
    if station.name.ends_with(" (International)") && !name.ends_with("(International)") {
        name.push_str(" (International)");
    }
    if name != station.name && ai::EXAMPLE_NAMES.contains(&name.as_str()) {
        warn!(
            provider = provider.as_str(),
            old = %station.name,
            new = %name,
            "AI echoed a few-shot example name; name override dropped"
        );
        name.clear();
    }
    if collapses_brand(&station.name, &name) {
        warn!(
            provider = provider.as_str(),
            old = %station.name,
            new = %name,
            "AI collapsed a variant stream to its parent brand; name override dropped"
        );
        name.clear();
    }
    Entry {
        provider,
        provider_id,
        source_hash: hash,
        name: (!name.is_empty() && name != station.name).then_some(name),
        tags: (!assessment.tags.is_empty() && assessment.tags != station.tags)
            .then(|| assessment.tags.clone()),
        description: assessment
            .description
            .as_deref()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty() && Some(d.as_str()) != station.description.as_deref()),
        reject: false,
    }
}

/// True when the new name is the old name minus a short trailing variant
/// (98FM Dance → 98FM, CBC Radio One: Grand Falls → CBC Radio One). Removing
/// a slogan is legitimate cleanup, but a slogan is a separator followed by a
/// phrase — a separator followed by one or two words is a variant or city
/// marker (France Musique - Classique Plus), which is identity, not slogan.
fn collapses_brand(old: &str, new: &str) -> bool {
    if new.is_empty() {
        return false;
    }
    let old = old.trim();
    let Some(rest) = old.strip_prefix(new) else {
        return false;
    };
    let words = rest
        .split_whitespace()
        .filter(|w| !w.chars().all(|c| matches!(c, ':' | '-' | '–' | '|' | '•')))
        .count();
    let rest = rest.trim();
    !rest.is_empty() && words <= 2
}

fn key(station: &Station) -> (String, String) {
    let id = match station.provider_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => station.stream_url.clone(),
    };
    (station.provider.clone(), id)
}

/// Stable FNV-1a hash of the provider-supplied fields that feed the model.
/// A change means the station needs re-assessment.
pub fn source_hash(station: &Station) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0x1f; // field separator
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    eat(station.name.as_bytes());
    eat(station.country_code.as_deref().unwrap_or("").as_bytes());
    // Tags are deliberately excluded: by the time stations reach this code
    // they carry Radio Browser enrichment, which drifts with votes and server
    // rotation. Hashing tags would re-assess stations weekly for that noise.
    eat(station.description.as_deref().unwrap_or("").as_bytes());
    format!("{hash:016x}")
}

fn load() -> Vec<Entry> {
    let Ok(source) = std::fs::read_to_string(OVERLAY_PATH) else {
        return Vec::new();
    };
    match toml::from_str::<OverlayFile>(&source) {
        Ok(file) => file.stations,
        Err(e) => {
            warn!(error = %e, "Could not parse enrichment overlay; ignoring it");
            Vec::new()
        }
    }
}

fn save(stations: Vec<Entry>) -> anyhow::Result<()> {
    let file = OverlayFile { stations };
    let body = toml::to_string_pretty(&file)?;
    let header = "# AI enrichment overlay. Generated by `cargo run -- enrich-overlay`;\n\
                  # applied deterministically by the nightly build. Edit by hand freely —\n\
                  # entries survive until the station's source_hash changes.\n\n";
    std::fs::write(OVERLAY_PATH, format!("{header}{body}"))?;
    Ok(())
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
            country_code: Some("MX".to_string()),
            tags: vec!["pop".to_string()],
            description: None,
            provider: provider.to_string(),
            provider_id: Some(id.to_string()),
            trusted: false,
        }
    }

    fn assessment(confidence: f32, accept: bool) -> ai::AiAssessment {
        ai::AiAssessment {
            accept,
            confidence,
            canonical_name: "Radio Cardinal".to_string(),
            country_code: "MX".to_string(),
            tags: vec!["latin".to_string()],
            description: Some("Mexican commercial radio station.".to_string()),
            logo_url: None,
            risks: vec![],
            reason: "test".to_string(),
        }
    }

    #[test]
    fn source_hash_is_stable_and_sensitive() {
        let a = station("curated", "1", "Radio Centro");
        let b = station("curated", "1", "Radio Centro");
        assert_eq!(source_hash(&a), source_hash(&b));

        let mut c = station("curated", "1", "Radio Centro");
        c.name = "RADIO CENTRO: Calidad".to_string();
        assert_ne!(source_hash(&a), source_hash(&c));
    }

    #[test]
    fn tag_drift_does_not_change_source_hash() {
        let a = station("curated", "1", "Radio Centro");
        let mut b = station("curated", "1", "Radio Centro");
        b.tags = vec!["jazz".to_string(), "funk".to_string()];
        assert_eq!(source_hash(&a), source_hash(&b));
    }

    #[test]
    fn confident_assessment_produces_overrides() {
        let s = station("curated", "1", "RADIO CENTRO: Calidad En Tu Vida");
        let entry = entry_from(&s, &assessment(0.9, true));
        assert_eq!(entry.name.as_deref(), Some("Radio Cardinal"));
        assert_eq!(entry.tags.as_deref(), Some(&["latin".to_string()][..]));
        assert!(!entry.reject);
    }

    #[test]
    fn low_confidence_records_hash_only() {
        let s = station("curated", "1", "Radio Centro");
        let entry = entry_from(&s, &assessment(0.4, true));
        assert!(entry.name.is_none());
        assert!(entry.tags.is_none());
        assert!(entry.description.is_none());
        assert!(!entry.reject);
    }

    #[test]
    fn rejection_only_applies_to_untrusted() {
        let s = station("curated", "1", "JUNK 123");
        assert!(entry_from(&s, &assessment(0.9, false)).reject);

        let mut trusted = station("bbc", "one", "BBC Radio 1");
        trusted.trusted = true;
        assert!(!entry_from(&trusted, &assessment(0.9, false)).reject);
    }

    #[test]
    fn overlay_roundtrips_through_toml() {
        let entry = Entry {
            provider: "curated".to_string(),
            provider_id: "https://example.com/1".to_string(),
            source_hash: "abc123".to_string(),
            name: Some("Radio Centro".to_string()),
            tags: Some(vec!["latin".to_string()]),
            description: None,
            reject: false,
        };
        let file = OverlayFile {
            stations: vec![entry],
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        assert!(!toml_str.contains("reject"));
        let back: OverlayFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.stations.len(), 1);
        assert_eq!(back.stations[0].name.as_deref(), Some("Radio Centro"));
    }

    #[test]
    fn collapses_brand_detects_variant_stripping() {
        assert!(collapses_brand("98FM Dance", "98FM"));
        assert!(collapses_brand("Absolute Radio 90s", "Absolute Radio"));
        assert!(collapses_brand(
            "BBC Radio One (International)",
            "BBC Radio One"
        ));
        // Slogan removal after a separator is legitimate cleanup.
        assert!(!collapses_brand(
            "Radio Centro: Calidad En Tu Vida",
            "Radio Centro"
        ));
        assert!(!collapses_brand(
            "SWR4 Webradio – Der Sound einer Ära",
            "SWR4 Webradio"
        ));
        // Different casing / real renames are not collapses.
        assert!(!collapses_brand("BBC Radio One", "BBC Radio 1"));
        assert!(!collapses_brand("1LIVE Top Hits ", "1LIVE Top Hits"));
    }

    #[test]
    fn collapses_brand_catches_city_and_variant_separators() {
        // Separator + one/two words is a city or variant marker, not a slogan.
        assert!(collapses_brand(
            "CBC Radio One: Grand Falls",
            "CBC Radio One"
        ));
        assert!(collapses_brand(
            "France Musique - Classique Plus",
            "France Musique"
        ));
        // A real slogan (3+ words) may still be stripped.
        assert!(!collapses_brand(
            "Radio Centro: Calidad En Tu Vida",
            "Radio Centro"
        ));
        assert!(!collapses_brand("Anything", ""));
    }

    #[test]
    fn example_name_echo_is_dropped() {
        let s = station("bauer", "tfa", "All Irish");
        let mut a = assessment(0.9, true);
        a.canonical_name = "Sunrise 106 OldSkool".to_string();
        let entry = entry_from(&s, &a);
        assert!(entry.name.is_none());
    }

    #[test]
    fn colliding_name_overrides_are_dropped() {
        let stations = vec![
            station("radio-france", "4", "France Musique"),
            station("radio-france", "402", "France Musique - Classique Plus"),
            station("radio-france", "401", "France Musique - Classique Easy"),
        ];
        let mut entries: HashMap<(String, String), Entry> = HashMap::new();
        entries.insert(
            ("radio-france".to_string(), "402".to_string()),
            Entry {
                provider: "radio-france".to_string(),
                provider_id: "402".to_string(),
                source_hash: "h".to_string(),
                name: Some("France Musique".to_string()),
                tags: None,
                description: None,
                reject: false,
            },
        );
        entries.insert(
            ("radio-france".to_string(), "401".to_string()),
            Entry {
                provider: "radio-france".to_string(),
                provider_id: "401".to_string(),
                source_hash: "h".to_string(),
                name: Some("France Musique Easy".to_string()),
                tags: None,
                description: None,
                reject: false,
            },
        );
        drop_colliding_names(&stations, &mut entries);
        // 402's rename collides with the real France Musique: dropped.
        assert!(
            entries[&("radio-france".to_string(), "402".to_string())]
                .name
                .is_none()
        );
        // 401's rename is unique: kept.
        assert!(
            entries[&("radio-france".to_string(), "401".to_string())]
                .name
                .is_some()
        );
    }

    #[test]
    fn collapsed_name_override_is_dropped() {
        let s = station("bauer", "98D", "98FM Dance");
        let mut a = assessment(0.9, true);
        a.canonical_name = "98FM".to_string();
        let entry = entry_from(&s, &a);
        assert!(entry.name.is_none());
        assert!(entry.tags.is_some());
    }

    #[test]
    fn international_suffix_is_preserved() {
        let s = station("bbc", "bbc_radio_one_int", "BBC Radio One (International)");
        let mut a = assessment(0.9, true);
        a.canonical_name = "BBC Radio 1".to_string();
        let entry = entry_from(&s, &a);
        assert_eq!(entry.name.as_deref(), Some("BBC Radio 1 (International)"));
    }
}

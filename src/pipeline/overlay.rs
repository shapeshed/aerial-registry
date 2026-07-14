use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::station::Station;

const OVERLAY_PATH: &str = "enrichment.toml";
const RADIO_BROWSER_OVERLAY_DIR: &str = "overlays/radio-browser";

/// One hand-written correction, keyed by the station's cross-run identity.
///
/// `enrichment.toml` is committed: the nightly build applies it with no model
/// or network dependency. Edit it directly to add or change a correction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    // Radio Browser only: a manual correction for a wrong/dead stream URL or
    // a broken logo — see `overlays/radio-browser/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
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
    let mut entries = load();
    entries.extend(load_radio_browser_overlays());
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
            if let Some(stream_url) = &entry.stream_url {
                station.stream_url = stream_url.clone();
            }
            if let Some(logo_url) = &entry.logo_url {
                station.logo_url = Some(logo_url.clone());
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

fn key(station: &Station) -> (String, String) {
    let id = match station.provider_id.as_deref() {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => station.stream_url.clone(),
    };
    (station.provider.clone(), id)
}

/// Stable FNV-1a hash of the provider-supplied fields an entry corrects.
/// A change means the underlying station data moved on since the entry was
/// written — `apply()` still applies it, just logs it as stale for review.
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

/// Human-edited corrections for Radio Browser's long tail, one TOML file
/// per country under `overlays/radio-browser/` so a fix only ever touches a
/// small, single-country diff — same `Entry` shape as `enrichment.toml`,
/// merged into the same `apply()` pass. Missing directory (the common case
/// until someone adds a first correction) is not an error.
fn load_radio_browser_overlays() -> Vec<Entry> {
    load_radio_browser_overlays_from(RADIO_BROWSER_OVERLAY_DIR)
}

fn load_radio_browser_overlays_from(dir: &str) -> Vec<Entry> {
    let dir = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            warn!(path = %path.display(), "Could not read radio-browser overlay file");
            continue;
        };
        match toml::from_str::<OverlayFile>(&source) {
            Ok(file) => entries.extend(file.stations),
            Err(e) => {
                warn!(path = %path.display(), error = %e, "Could not parse radio-browser overlay; ignoring it");
            }
        }
    }
    entries
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
    fn overlay_roundtrips_through_toml() {
        let entry = Entry {
            provider: "curated".to_string(),
            provider_id: "https://example.com/1".to_string(),
            source_hash: "abc123".to_string(),
            name: Some("Radio Centro".to_string()),
            tags: Some(vec!["latin".to_string()]),
            description: None,
            reject: false,
            ..Default::default()
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

    fn temp_overlay_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aerial-registry-overlay-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn radio_browser_overlays_merge_across_country_files() {
        let dir = temp_overlay_dir("merge");
        std::fs::write(
            dir.join("GB.toml"),
            r#"[[station]]
provider = "radio-browser"
provider_id = "uuid-1"
source_hash = "h1"
stream_url = "https://fixed.example.com/gb-1"
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("DE.toml"),
            r#"[[station]]
provider = "radio-browser"
provider_id = "uuid-2"
source_hash = "h2"
reject = true
"#,
        )
        .unwrap();

        let entries = load_radio_browser_overlays_from(dir.to_str().unwrap());
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(entries.len(), 2);
        let gb = entries.iter().find(|e| e.provider_id == "uuid-1").unwrap();
        assert_eq!(
            gb.stream_url.as_deref(),
            Some("https://fixed.example.com/gb-1")
        );
        let de = entries.iter().find(|e| e.provider_id == "uuid-2").unwrap();
        assert!(de.reject);
    }

    #[test]
    fn radio_browser_overlays_ignore_non_toml_files() {
        let dir = temp_overlay_dir("ignore-non-toml");
        std::fs::write(dir.join("README.md"), "not a station file").unwrap();

        let entries = load_radio_browser_overlays_from(dir.to_str().unwrap());
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(entries.is_empty());
    }

    #[test]
    fn radio_browser_overlays_missing_directory_is_not_an_error() {
        assert!(load_radio_browser_overlays_from("/nonexistent/overlays/radio-browser").is_empty());
    }
}

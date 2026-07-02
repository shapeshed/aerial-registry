use tracing::debug;

use crate::station::Station;

pub fn dedup(stations: Vec<Station>) -> Vec<Station> {
    let input_count = stations.len();
    // Broadcaster-direct providers ranked highest; curated entries and
    // aggregators lowest. Listed by exception so a newly added direct
    // provider ranks correctly without touching this table.
    let provider_rank = |p: &str| match p {
        "curated" | "radio-browser" => 1u8,
        _ => 0u8,
    };

    let mut stations = stations;
    // Sort so higher-priority providers come first within each URL group.
    stations.sort_by(|a, b| {
        provider_rank(&a.provider)
            .cmp(&provider_rank(&b.provider))
            .then(a.name.cmp(&b.name))
    });

    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out: Vec<Station> = Vec::new();

    for station in stations {
        let key = normalise_url(&station.stream_url);
        if let Some(&idx) = seen.get(&key) {
            // Merge tags and fill any missing optional fields from the duplicate.
            let existing = &mut out[idx];
            for tag in &station.tags {
                if !existing.tags.contains(tag) {
                    existing.tags.push(tag.clone());
                }
            }
            if existing.logo_url.is_none() {
                existing.logo_url = station.logo_url;
            }
            if existing.description.is_none() {
                existing.description = station.description;
            }
            if existing.country.is_none() {
                existing.country = station.country;
            }
            if existing.country_code.is_none() {
                existing.country_code = station.country_code;
            }
            debug!(url = %station.stream_url, kept = %existing.name, "Deduplicated station");
        } else {
            seen.insert(key, out.len());
            out.push(station);
        }
    }

    tracing::info!(
        before = input_count,
        after = out.len(),
        "Deduplication complete"
    );
    out
}

pub fn normalise_url(url: &str) -> String {
    url.to_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('?')
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(provider: &str, name: &str, url: &str) -> Station {
        Station {
            name: name.to_string(),
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
    fn direct_provider_wins_over_curated() {
        let out = dedup(vec![
            station("curated", "AAA Radio", "https://example.com/stream"),
            station("radio-france", "franceinfo", "https://example.com/stream"),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, "radio-france");
    }

    #[test]
    fn normalise_strips_scheme_query_and_trailing_slash() {
        assert_eq!(
            normalise_url("https://Example.com/stream/?id=1"),
            "example.com/stream"
        );
        assert_eq!(
            normalise_url("http://example.com/stream?id=1"),
            "example.com/stream"
        );
        assert_eq!(
            normalise_url("https://example.com/stream/"),
            "example.com/stream"
        );
    }

    #[test]
    fn duplicate_fills_missing_fields() {
        let mut a = station("bbc", "World Service", "https://example.com/ws");
        a.tags = vec!["News".to_string()];
        let mut b = station("curated", "BBC WS", "http://example.com/ws/");
        b.tags = vec!["Talk".to_string()];
        b.country = Some("United Kingdom".to_string());
        b.country_code = Some("GB".to_string());
        b.logo_url = Some("https://example.com/logo.png".to_string());

        let out = dedup(vec![a, b]);
        assert_eq!(out.len(), 1);
        let kept = &out[0];
        assert_eq!(kept.provider, "bbc");
        assert_eq!(kept.tags, vec!["News".to_string(), "Talk".to_string()]);
        assert_eq!(kept.country_code.as_deref(), Some("GB"));
        assert!(kept.logo_url.is_some());
    }
}

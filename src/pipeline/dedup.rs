use tracing::debug;

use crate::station::Station;

// Broadcaster-direct providers ranked highest; curated entries and
// aggregators lowest. Listed by exception so a newly added direct provider
// ranks correctly without touching this table.
fn provider_rank(p: &str) -> u8 {
    match p {
        "curated" | "radio-browser" => 1,
        _ => 0,
    }
}

pub fn dedup(stations: Vec<Station>) -> Vec<Station> {
    let input_count = stations.len();

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
            debug!(url = %station.stream_url, kept = %out[idx].name, "Deduplicated station");
            merge_into(&mut out[idx], station);
        } else {
            seen.insert(key, out.len());
            out.push(station);
        }
    }
    let after_url_dedup = out.len();

    let out = drop_aggregator_duplicates(out);

    tracing::info!(
        before = input_count,
        after_url_dedup,
        after = out.len(),
        "Deduplication complete"
    );
    out
}

// Fills any missing optional fields on `existing` from `duplicate`, and unions tags.
fn merge_into(existing: &mut Station, duplicate: Station) {
    for tag in &duplicate.tags {
        if !existing.tags.contains(tag) {
            existing.tags.push(tag.clone());
        }
    }
    if existing.logo_url.is_none() {
        existing.logo_url = duplicate.logo_url;
    }
    if existing.description.is_none() {
        existing.description = duplicate.description;
    }
    if existing.country.is_none() {
        existing.country = duplicate.country;
    }
    if existing.country_code.is_none() {
        existing.country_code = duplicate.country_code;
    }
}

// Stream-URL matching alone misses same-station duplicates when a broadcaster
// publishes multiple CDN/bitrate mirrors and radio-browser has indexed a
// different one under its own URL: same station, non-matching URL. Once a
// `trusted` (first-party broadcaster) entry exists for a (name, country_code)
// pair, any `radio-browser` entry sharing that exact pair is redundant
// aggregator noise, so drop it regardless of stream URL. Groups with no
// trusted entry are left untouched — radio-browser is often the only source
// for a station, and its entries must not be merged away just for sharing a
// generic name. A second trusted entry in the same group (two distinct
// first-party providers, e.g. by coincidence) is also left alone: this pass
// only ever drops radio-browser entries, never trusted-vs-trusted.
//
// Scoped to `radio-browser` specifically, not "anything untrusted": `curated`
// is also `trusted: false` (that flag only means "skip liveness checks", not
// "not editorially chosen") — these are deliberately hand-picked and must
// never be dropped just for sharing a name with a broadcaster's own feed.
fn drop_aggregator_duplicates(mut stations: Vec<Station>) -> Vec<Station> {
    let mut groups: std::collections::HashMap<(String, String), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, s) in stations.iter().enumerate() {
        // Country code is required to key a group: without it we can't rule out
        // merging unrelated same-named stations from different countries.
        if let Some(cc) = s.country_code.as_deref().filter(|cc| !cc.is_empty()) {
            groups
                .entry((s.name.to_lowercase(), cc.to_lowercase()))
                .or_default()
                .push(i);
        }
    }

    let mut drop: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut merges: Vec<(usize, usize)> = Vec::new();

    for indices in groups.values() {
        let Some(&keep_idx) = indices.iter().find(|&&i| stations[i].trusted) else {
            continue; // No trusted direct entry in this group — leave as-is.
        };
        for &i in indices {
            if i != keep_idx && stations[i].provider == "radio-browser" {
                drop.insert(i);
                merges.push((keep_idx, i));
            }
        }
    }

    for (keep_idx, drop_idx) in merges {
        debug!(
            dropped = %stations[drop_idx].name,
            provider = %stations[drop_idx].provider,
            kept_provider = %stations[keep_idx].provider,
            "Dropped aggregator duplicate of trusted provider entry"
        );
        let duplicate = stations[drop_idx].clone();
        merge_into(&mut stations[keep_idx], duplicate);
    }

    stations
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !drop.contains(i))
        .map(|(_, s)| s)
        .collect()
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

    fn station_with_country(provider: &str, name: &str, url: &str, country_code: &str) -> Station {
        let mut s = station(provider, name, url);
        s.country_code = Some(country_code.to_string());
        // Real providers set this explicitly; mirror that here too, since
        // pass 2's `keep_idx` selection keys on `trusted`.
        s.trusted = provider != "radio-browser" && provider != "curated";
        s
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

    #[test]
    fn trusted_provider_absorbs_aggregator_mirrors_with_different_urls() {
        // Same station name/country, but three distinct radio-browser mirror
        // URLs that don't textually match the bbc entry's URL or each other —
        // exactly the BBC Radio 4 case that motivated this pass.
        let mut bbc = station_with_country(
            "bbc",
            "BBC Radio 4",
            "https://as-hls-uk-live.akamaized.net/bbc_radio_fourfm-320.m3u8",
            "GB",
        );
        bbc.tags = vec!["news".to_string()];

        let mut mirror_a = station_with_country(
            "radio-browser",
            "BBC Radio 4",
            "https://as-hls-ww-live.akamaized.net/bbc_radio_fourfm-128.m3u8",
            "GB",
        );
        mirror_a.tags = vec!["talk".to_string()];
        let mirror_b = station_with_country(
            "radio-browser",
            "BBC Radio 4",
            "https://as-hls-ww-live.akamaized.net/bbc_radio_fourfm-320.m3u8",
            "GB",
        );

        let out = dedup(vec![bbc, mirror_a, mirror_b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, "bbc");
        assert_eq!(out[0].tags, vec!["news".to_string(), "talk".to_string()]);
    }

    #[test]
    fn aggregator_only_group_is_left_alone() {
        // No trusted direct provider for this name/country — both radio-browser
        // mirrors must survive, since aggregator data may be the only source.
        let out = dedup(vec![
            station_with_country(
                "radio-browser",
                "Some Community Station",
                "https://a.example/x",
                "FR",
            ),
            station_with_country(
                "radio-browser",
                "Some Community Station",
                "https://b.example/y",
                "FR",
            ),
        ]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn curated_station_survives_alongside_trusted_duplicate() {
        // Regression: `curated` is `trusted: false` too (that flag only means
        // "skip liveness checks"), but it's deliberately hand-picked and must
        // never be dropped just for sharing a name/country with a trusted
        // broadcaster's own feed — unlike radio-browser, which should be.
        let out = dedup(vec![
            station_with_country("bbc", "BBC Radio 1", "https://bbc.example/r1", "GB"),
            station_with_country("curated", "BBC Radio 1", "https://curated.example/r1", "GB"),
        ]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn same_name_different_country_is_not_merged() {
        // Same station name, different countries — must never merge across
        // borders even though both are "Radio 4".
        let out = dedup(vec![
            station_with_country("bbc", "Radio 4", "https://bbc.example/stream", "GB"),
            station_with_country(
                "radio-browser",
                "Radio 4",
                "https://lv.example/stream",
                "LV",
            ),
        ]);
        assert_eq!(out.len(), 2);
    }
}

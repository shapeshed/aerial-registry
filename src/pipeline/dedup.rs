use tracing::debug;

use crate::station::Station;

pub fn dedup(stations: Vec<Station>) -> Vec<Station> {
    let input_count = stations.len();
    // Broadcaster-direct providers ranked highest; aggregators lowest.
    let provider_rank = |p: &str| match p {
        "abc" | "bbc" | "bauer" | "dr" | "global" | "npo" | "nrk" | "orf" | "rai" | "rtbf"
        | "rte" | "rtp" | "sr" | "wireless" => 0u8,
        _ => 1u8,
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

fn normalise_url(url: &str) -> String {
    url.to_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .split('?')
        .next()
        .unwrap_or("")
        .to_owned()
}

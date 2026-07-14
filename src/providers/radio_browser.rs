use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use crate::pipeline::dedup::normalise_url;
use crate::radio_browser_client::{discover_servers, with_retry};
use crate::station::Station;

const PAGE_SIZE: u32 = 5_000;
// Safety cap against a runaway loop, not a real limit — Radio Browser's
// full catalog is well under this many pages at PAGE_SIZE each.
const MAX_PAGES: u32 = 40;

#[derive(Deserialize)]
struct RbStation {
    stationuuid: String,
    name: String,
    url_resolved: String,
    favicon: String,
    tags: String,
    country: String,
    countrycode: String,
    #[serde(default)]
    votes: i64,
    #[serde(default)]
    clickcount: i64,
}

/// Bulk long-tail discovery: everything a trusted broadcaster-direct
/// provider doesn't cover. Radio Browser is community-submitted and
/// explicitly untrusted (`trusted: false`) — `pipeline::dedup` drops any
/// entry here that duplicates a trusted provider's station by
/// `(name, country_code)`, and known-bad entries are corrected or excluded
/// via `overlays/radio-browser/<COUNTRY>.toml` rather than hand-edited here.
pub async fn discover(client: &Client) -> Vec<Station> {
    let servers = match discover_servers(client).await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                provider = "radio-browser",
                error = %e,
                "Server discovery failed — no stations this run"
            );
            return vec![];
        }
    };

    let mut raw = Vec::new();
    for page in 0..MAX_PAGES {
        let offset = page * PAGE_SIZE;
        let context = format!("bulk fetch offset={offset}");
        let batch = with_retry(&servers, &context, |server| async move {
            fetch_page(client, &server, offset, PAGE_SIZE).await
        })
        .await;

        let Some(batch) = batch else {
            // Exhausted retries for this page: stop here and return whatever
            // was collected so far rather than looping on a dead upstream.
            // The registry-level guard decides whether a partial result this
            // small is worth publishing or carrying the previous run forward.
            warn!(
                provider = "radio-browser",
                offset,
                collected = raw.len(),
                "Giving up mid-pagination"
            );
            break;
        };

        let page_len = batch.len();
        raw.extend(batch);

        if page_len < PAGE_SIZE as usize {
            break; // Short page: this was the last one.
        }
    }

    let before = raw.len();
    let deduped = dedup_within_provider(raw);
    let stations: Vec<Station> = deduped.into_iter().filter_map(to_station).collect();

    info!(
        provider = "radio-browser",
        fetched = before,
        after_internal_dedup = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Radio Browser itself contains duplicates: multiple community submissions
/// of the same physical stream under slightly different names/entries. Group
/// by normalised stream URL and keep the most-corroborated record — highest
/// `votes * 2 + clickcount` — borrowing its favicon from a loser if it lacks
/// one itself. This is separate from `pipeline::dedup`, which handles
/// cross-provider duplicates against trusted broadcasters.
fn dedup_within_provider(stations: Vec<RbStation>) -> Vec<RbStation> {
    fn score(s: &RbStation) -> i64 {
        s.votes.saturating_mul(2).saturating_add(s.clickcount)
    }

    let mut best: HashMap<String, RbStation> = HashMap::new();
    for station in stations {
        let key = normalise_url(&station.url_resolved);
        match best.get_mut(&key) {
            None => {
                best.insert(key, station);
            }
            Some(existing) => {
                if station.favicon.trim().starts_with("http")
                    && !existing.favicon.trim().starts_with("http")
                {
                    existing.favicon = station.favicon.clone();
                }
                if score(&station) > score(existing) {
                    let favicon = if existing.favicon.trim().starts_with("http") {
                        existing.favicon.clone()
                    } else {
                        station.favicon.clone()
                    };
                    let mut winner = station;
                    winner.favicon = favicon;
                    *existing = winner;
                }
            }
        }
    }
    best.into_values().collect()
}

async fn fetch_page(
    client: &Client,
    server: &str,
    offset: u32,
    limit: u32,
) -> anyhow::Result<Vec<RbStation>> {
    let url = format!("https://{server}/json/stations/search");
    client
        .get(&url)
        .query(&[
            ("hidebroken", "true".to_string()),
            ("order", "name".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ])
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("request: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("status: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse: {e}"))
}

/// Mechanical filtering only — no vote/bitrate/favicon bar. Breadth is the
/// point of this provider; `pipeline::liveness` and its three-strike
/// hysteresis prune genuinely dead streams over time, and a missing logo
/// already renders fine as a placeholder in the app.
fn to_station(s: RbStation) -> Option<Station> {
    let stream_url = s.url_resolved.trim();
    if stream_url.is_empty() {
        return None;
    }
    let lower = stream_url.to_ascii_lowercase();
    if lower.ends_with(".pls") || lower.ends_with(".m3u") {
        return None; // Playlist container, not a directly playable stream.
    }

    let favicon = s.favicon.trim();
    let logo_url = favicon
        .to_ascii_lowercase()
        .starts_with("http")
        .then(|| favicon.to_string());

    Some(Station {
        name: clean_name(&s.name),
        stream_url: stream_url.to_string(),
        logo_url,
        country: non_empty(s.country),
        country_code: non_empty(s.countrycode),
        tags: split_tags(&s.tags),
        description: None,
        provider: "radio-browser".into(),
        provider_id: Some(s.stationuuid),
        trusted: false,
    })
}

fn non_empty(s: String) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Raw, free-form tags — not validated against `pipeline::tags`'s taxonomy
/// (that's for stations enriched via a name lookup; Radio Browser stations
/// carry their own). Lowercased to match the casing every other provider's
/// tags use, and deduplicated case-insensitively.
fn split_tags(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in raw.split(',') {
        let tag = tag.trim().to_ascii_lowercase();
        if !tag.is_empty() && !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

const CODEC_WORDS: &[&str] = &["mp3", "aac", "aac+", "ogg", "flac", "wma", "opus"];
const QUALITY_WORDS: &[&str] = &["hq", "hd", "high", "quality", "medium", "low", "bitrate"];

/// A trailing suffix like "[MP3]", "(128k)", "| MP3 128k", "- AAC HD 256k",
/// or a bare "AAC 256k"/"HQ" is codec/bitrate noise appended by whatever
/// submitted the station, not part of its name. Strip it, repeatedly, since
/// stations sometimes carry more than one such suffix chained together.
fn clean_name(raw: &str) -> String {
    let mut name = raw.trim().to_string();
    loop {
        let before = name.clone();

        if let Some(rest) = strip_trailing_wrapped(&name, '[', ']') {
            name = rest;
        } else if let Some(rest) = strip_trailing_wrapped(&name, '(', ')') {
            name = rest;
        } else if let Some((head, tail)) = name.rsplit_once('|') {
            if is_noise_phrase(tail) {
                name = head.trim_end().to_string();
            }
        } else if let Some((head, tail)) = name.rsplit_once('-') {
            if is_noise_phrase(tail) {
                name = head.trim_end().to_string();
            }
        } else {
            let words: Vec<&str> = name.split_whitespace().collect();
            let mut keep = words.len();
            while keep > 0 && is_noise_token(words[keep - 1]) {
                keep -= 1;
            }
            if keep < words.len() && keep > 0 {
                name = words[..keep].join(" ");
            }
        }

        if name == before {
            break;
        }
    }
    name.trim().to_string()
}

fn strip_trailing_wrapped(name: &str, open: char, close: char) -> Option<String> {
    let trimmed = name.trim_end();
    if !trimmed.ends_with(close) {
        return None;
    }
    let open_idx = trimmed.rfind(open)?;
    let inner = &trimmed[open_idx + open.len_utf8()..trimmed.len() - close.len_utf8()];
    is_noise_phrase(inner).then(|| trimmed[..open_idx].trim_end().to_string())
}

/// True only if every word in the phrase is codec/quality/bitrate noise —
/// so "AAC 256k" strips but a real trailing word never does.
fn is_noise_phrase(phrase: &str) -> bool {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    !words.is_empty() && words.iter().all(|w| is_noise_token(w))
}

fn is_noise_token(token: &str) -> bool {
    let t = token.trim().trim_matches(|c: char| !c.is_alphanumeric());
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if CODEC_WORDS.contains(&lower.as_str()) || QUALITY_WORDS.contains(&lower.as_str()) {
        return true;
    }
    // Bitrate-like: digits followed by "k" or "kbps", e.g. "128k", "256kbps".
    let digits: String = lower.chars().take_while(|c| c.is_ascii_digit()).collect();
    !digits.is_empty() && matches!(&lower[digits.len()..], "k" | "kbps")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(url_resolved: &str) -> RbStation {
        RbStation {
            stationuuid: "abc-123".to_string(),
            name: "Test FM".to_string(),
            url_resolved: url_resolved.to_string(),
            favicon: "https://example.com/logo.png".to_string(),
            tags: "pop, rock, ".to_string(),
            country: "United Kingdom".to_string(),
            countrycode: "GB".to_string(),
            votes: 0,
            clickcount: 0,
        }
    }

    #[test]
    fn maps_fields_and_marks_untrusted() {
        let out = to_station(station("https://example.com/stream")).unwrap();
        assert_eq!(out.provider, "radio-browser");
        assert_eq!(out.provider_id.as_deref(), Some("abc-123"));
        assert!(!out.trusted);
        assert_eq!(out.stream_url, "https://example.com/stream");
        assert_eq!(
            out.logo_url.as_deref(),
            Some("https://example.com/logo.png")
        );
        assert_eq!(out.country_code.as_deref(), Some("GB"));
        assert_eq!(out.tags, vec!["pop".to_string(), "rock".to_string()]);
    }

    #[test]
    fn tags_are_lowercased_and_deduplicated() {
        assert_eq!(
            split_tags("Pop, POP, Rock, pop"),
            vec!["pop".to_string(), "rock".to_string()]
        );
    }

    #[test]
    fn drops_entries_with_no_resolved_url() {
        assert!(to_station(station("")).is_none());
        assert!(to_station(station("   ")).is_none());
    }

    #[test]
    fn drops_playlist_container_links() {
        assert!(to_station(station("https://example.com/stream.pls")).is_none());
        assert!(to_station(station("https://example.com/STREAM.M3U")).is_none());
    }

    #[test]
    fn keeps_stations_with_no_favicon() {
        let mut s = station("https://example.com/stream");
        s.favicon = "".to_string();
        let out = to_station(s).unwrap();
        assert_eq!(out.logo_url, None);
    }

    #[test]
    fn clean_name_strips_bracketed_and_parenthesised_codec_suffixes() {
        assert_eq!(clean_name("Radio One [MP3]"), "Radio One");
        assert_eq!(clean_name("Radio One (128k)"), "Radio One");
        assert_eq!(clean_name("Radio One (HQ)"), "Radio One");
        assert_eq!(clean_name("Radio One (Medium Bitrate)"), "Radio One");
    }

    #[test]
    fn clean_name_strips_pipe_and_dash_suffixes() {
        assert_eq!(clean_name("Radio One | MP3 128k"), "Radio One");
        assert_eq!(clean_name("Radio One - MP3"), "Radio One");
        assert_eq!(clean_name("Radio One - AAC HD 256k"), "Radio One");
    }

    #[test]
    fn clean_name_strips_bare_trailing_tokens() {
        assert_eq!(clean_name("Radio One AAC 256k"), "Radio One");
        assert_eq!(clean_name("Radio One HQ"), "Radio One");
        assert_eq!(clean_name("Radio One MP3 128kbps"), "Radio One");
    }

    #[test]
    fn clean_name_strips_chained_suffixes() {
        assert_eq!(clean_name("Radio One [MP3] (128k)"), "Radio One");
    }

    #[test]
    fn clean_name_leaves_real_names_alone() {
        assert_eq!(clean_name("Radio One"), "Radio One");
        assert_eq!(clean_name("98FM Dance"), "98FM Dance");
        assert_eq!(clean_name("Café del Mar"), "Café del Mar");
        // "Live" isn't codec/quality/bitrate noise — must not be stripped.
        assert_eq!(clean_name("Radio One [Live]"), "Radio One [Live]");
    }

    #[test]
    fn dedup_within_provider_keeps_highest_scored_and_merges_favicon() {
        let mut a = station("https://example.com/stream");
        a.stationuuid = "a".to_string();
        a.votes = 10;
        a.clickcount = 5;
        a.favicon = "https://example.com/a-logo.png".to_string();

        let mut b = station("https://example.com/stream/"); // same URL, trailing slash
        b.stationuuid = "b".to_string();
        b.votes = 50;
        b.clickcount = 1;
        b.favicon = "".to_string(); // winner lacks a favicon of its own

        let out = dedup_within_provider(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].stationuuid, "b"); // higher score: 50*2+1 > 10*2+5
        assert_eq!(out[0].favicon, "https://example.com/a-logo.png"); // borrowed from a
    }

    #[test]
    fn dedup_within_provider_leaves_distinct_streams_alone() {
        let a = station("https://example.com/stream-a");
        let b = station("https://example.com/stream-b");
        let out = dedup_within_provider(vec![a, b]);
        assert_eq!(out.len(), 2);
    }
}

//! Discovers stations by enumerating the Icecast/SHOUTcast servers behind
//! Radio-Browser's stream hosts.
//!
//! Radio-Browser stores one URL per station; the server it points at usually
//! exposes more — additional mounts (Icecast) or stream IDs (SHOUTcast) — and
//! reports its *own* name/genre/bitrate, which is more accurate than RB's
//! community-submitted copy. This provider seeds the host list from RB,
//! enumerates each host, and emits stations from the server's metadata. Dedup
//! then merges them with RB (this provider outranks radio-browser, so the
//! authoritative metadata wins and RB's extra fields fill the gaps).
//!
//! A host is either Icecast (`/status-json.xsl`) or SHOUTcast
//! (`/stats?sid=N&json=1`), so both endpoint families are tried per host.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tracing::info;

use crate::http::Client;
use crate::station::{Station, StreamVariant, bitrate_from_url};

const CONCURRENCY: usize = 128;
const MAX_SHOUTCAST_SIDS: u32 = 8;

/// Server names that carry no information.
const JUNK_NAMES: &[&str] = &[
    "no name",
    "unnamed",
    "unknown",
    "none",
    "null",
    "(null)",
    "various",
    "my station name",
    "default stream",
    "test",
];

#[derive(Debug, Clone)]
struct Mount {
    name: String,
    genre: String,
    bitrate: Option<u32>,
    format: Option<String>,
    url: String,
    listeners: u64,
}

#[derive(Deserialize)]
struct IceStats {
    icestats: Option<IceStatsInner>,
}

#[derive(Deserialize)]
struct IceStatsInner {
    source: Option<SourceSet>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SourceSet {
    One(Source),
    Many(Vec<Source>),
}

#[derive(Deserialize)]
struct Source {
    #[serde(default)]
    server_name: Option<String>,
    #[serde(default)]
    listenurl: Option<String>,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    bitrate: Option<u32>,
    #[serde(default)]
    server_type: Option<String>,
    #[serde(default)]
    listeners: Option<u64>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    // Seed the host list from Radio-Browser (host:port of each stream URL).
    let rb = crate::providers::radio_browser::discover(client).await;
    let mut hosts: HashSet<String> = HashSet::new();
    for station in &rb {
        if let Some(host) = netloc(&station.stream_url)
            && !host.contains("do-not-publish")
        {
            hosts.insert(host);
        }
    }
    let hosts: Vec<String> = hosts.into_iter().collect();
    let total_hosts = hosts.len();
    info!(
        provider = "icecast-enum",
        hosts = total_hosts,
        "Seeded hosts from Radio-Browser"
    );

    let Ok(http) = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .user_agent("aerial-registry/0.1 (+icecast-enum)")
        .build()
    else {
        return vec![];
    };

    let mut stream = stream::iter(hosts)
        .map(|host| {
            let http = http.clone();
            async move {
                let mounts = enumerate(&http, &host).await;
                (host, mounts)
            }
        })
        .buffer_unordered(CONCURRENCY);

    let mut results: Vec<(String, Vec<Mount>)> = Vec::new();
    let mut processed = 0usize;
    let mut responded = 0usize;
    while let Some((host, mounts)) = stream.next().await {
        processed += 1;
        if !mounts.is_empty() {
            responded += 1;
        }
        if processed % 1000 == 0 {
            info!(
                provider = "icecast-enum",
                processed,
                total = total_hosts,
                responded,
                "Enumerating hosts"
            );
        }
        results.push((host, mounts));
    }
    info!(
        provider = "icecast-enum",
        processed, responded, "Host enumeration complete"
    );

    let mut groups: HashMap<(String, String), Group> = HashMap::new();
    for (host, mounts) in results {
        for mount in mounts {
            // Drop only truly empty mounts: a placeholder/mount-path name with no
            // genre, no bitrate and no listeners. Anything with a real signal —
            // including anonymous streams that have listeners — is kept.
            let name_is_placeholder = mount.name.starts_with('/') || is_junk_name(&mount.name);
            let has_signal =
                !mount.genre.trim().is_empty() || mount.bitrate.is_some() || mount.listeners > 0;
            if name_is_placeholder && !has_signal {
                continue;
            }
            let key = (host.clone(), name_key(&mount.name));
            let group = groups.entry(key).or_insert_with(|| Group {
                name: mount.name.clone(),
                tags: parse_tags(&mount.genre),
                primary_bitrate: 0,
                primary_url: mount.url.clone(),
                variants: Vec::new(),
            });
            for tag in parse_tags(&mount.genre) {
                if !group.tags.contains(&tag) {
                    group.tags.push(tag);
                }
            }
            let variant = StreamVariant {
                url: mount.url.clone(),
                bitrate: mount.bitrate.or_else(|| bitrate_from_url(&mount.url)),
                format: mount.format.clone(),
            };
            if !group.variants.iter().any(|v| v.url == variant.url) {
                group.variants.push(variant);
            }
            let bitrate = mount.bitrate.unwrap_or(0);
            if bitrate >= group.primary_bitrate {
                group.primary_bitrate = bitrate;
                group.primary_url = mount.url;
            }
        }
    }

    let mut stations: Vec<Station> = groups
        .into_values()
        .map(|group| Station {
            name: group.name,
            stream_url: group.primary_url,
            streams: if group.variants.len() > 1 {
                group.variants
            } else {
                Vec::new()
            },
            logo_url: None,
            country: None,
            country_code: None,
            tags: group.tags,
            description: None,
            provider: "icecast-enum".into(),
            provider_id: None,
            trusted: false,
        })
        .collect();
    stations.sort_by(|a, b| a.name.cmp(&b.name));
    info!(
        provider = "icecast-enum",
        count = stations.len(),
        "Discovered stations"
    );
    stations
}

struct Group {
    name: String,
    tags: Vec<String>,
    primary_bitrate: u32,
    primary_url: String,
    variants: Vec<StreamVariant>,
}

/// Enumerate a host: try the Icecast endpoint, then SHOUTcast. Cleartext HTTP
/// first — most are cleartext and it avoids paying the HTTPS timeout on dead
/// hosts.
async fn enumerate(http: &reqwest::Client, host: &str) -> Vec<Mount> {
    for scheme in ["http", "https"] {
        let mounts = icecast_mounts(http, scheme, host).await;
        if !mounts.is_empty() {
            return mounts;
        }
    }
    for scheme in ["http", "https"] {
        let mounts = shoutcast_mounts(http, scheme, host).await;
        if !mounts.is_empty() {
            return mounts;
        }
    }
    Vec::new()
}

async fn icecast_mounts(http: &reqwest::Client, scheme: &str, host: &str) -> Vec<Mount> {
    let url = format!("{scheme}://{host}/status-json.xsl");
    let Ok(response) = http.get(&url).send().await else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(stats) = response.json::<IceStats>().await else {
        return Vec::new();
    };
    let Some(sources) = stats.icestats.and_then(|inner| inner.source) else {
        return Vec::new();
    };
    let sources = match sources {
        SourceSet::One(source) => vec![source],
        SourceSet::Many(sources) => sources,
    };
    sources
        .into_iter()
        .filter_map(|source| {
            let listenurl = source.listenurl.filter(|u| u.starts_with("http"))?;
            Some(Mount {
                name: source.server_name.unwrap_or_default().trim().to_string(),
                genre: source.genre.unwrap_or_default(),
                bitrate: source.bitrate.filter(|b| *b > 0),
                format: source.server_type.map(|t| codec_name(&t)),
                url: public_url(scheme, host, &listenurl),
                listeners: source.listeners.unwrap_or(0),
            })
        })
        .collect()
}

async fn shoutcast_mounts(http: &reqwest::Client, scheme: &str, host: &str) -> Vec<Mount> {
    let mut mounts = Vec::new();
    for sid in 1..=MAX_SHOUTCAST_SIDS {
        let url = format!("{scheme}://{host}/stats?sid={sid}&json=1");
        let Ok(response) = http.get(&url).send().await else {
            break;
        };
        if !response.status().is_success() {
            break;
        }
        let Ok(value) = response.json::<serde_json::Value>().await else {
            break;
        };
        let name = value
            .get("servertitle")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let path = value
            .get("streampath")
            .and_then(|v| v.as_str())
            .unwrap_or("/");
        mounts.push(Mount {
            name,
            genre: value
                .get("servergenre")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            bitrate: value
                .get("bitrate")
                .and_then(|v| v.as_str())
                .and_then(|b| b.parse::<u32>().ok()),
            format: None,
            url: format!("{scheme}://{host}{path}"),
            listeners: value
                .get("currentlisteners")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        });
    }
    mounts
}

/// Rebuild a stream URL when the server reports an internal `listenurl` host
/// (localhost/private IP) — use the host we actually enumerated plus the path.
fn public_url(scheme: &str, host: &str, listenurl: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(listenurl) else {
        return listenurl.to_string();
    };
    match parsed.host_str() {
        Some(internal) if is_internal_host(internal) => {
            format!("{scheme}://{host}{}", parsed.path())
        }
        _ => listenurl.to_string(),
    }
}

fn is_internal_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host == "0.0.0.0" {
        return true;
    }
    if host.starts_with("127.") || host.starts_with("10.") || host.starts_with("192.168.") {
        return true;
    }
    if let Some(rest) = host.strip_prefix("172.")
        && let Some(second) = rest.split('.').next().and_then(|o| o.parse::<u8>().ok())
    {
        return (16..=31).contains(&second);
    }
    false
}

fn netloc(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn name_key(name: &str) -> String {
    name.to_ascii_lowercase()
        .split_whitespace()
        .filter(|word| {
            !word.contains("kbit")
                && !word.contains("kbps")
                && !matches!(*word, "mp3" | "aac" | "aacp" | "ogg" | "opus" | "hls")
        })
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

fn is_junk_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    name.is_empty() || JUNK_NAMES.contains(&name.as_str())
}

fn parse_tags(genre: &str) -> Vec<String> {
    let mut tags: Vec<String> = genre
        .split([',', ';', '/', '|'])
        .flat_map(|part| part.split_whitespace())
        .map(|word| word.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|word| word.len() >= 2 && !matches!(word.as_str(), "various" | "other" | "misc"))
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn codec_name(server_type: &str) -> String {
    match server_type.to_ascii_lowercase().as_str() {
        "audio/mpeg" => "mp3",
        "audio/aac" | "audio/aacp" | "audio/x-aac" => "aac",
        "audio/ogg" | "application/ogg" => "ogg",
        "audio/opus" => "opus",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netloc_keeps_the_port() {
        assert_eq!(netloc("http://host:8000/stream"), Some("host:8000".into()));
        assert_eq!(netloc("https://host/stream"), Some("host".into()));
    }

    #[test]
    fn groups_bitrate_variants_by_stripped_name() {
        assert_eq!(name_key("Radio X 128kbit"), name_key("Radio X 64kbit"));
        assert_ne!(name_key("Radio X"), name_key("Radio Y"));
    }

    #[test]
    fn rejects_junk_names() {
        assert!(is_junk_name("No Name"));
        assert!(is_junk_name("  "));
        assert!(!is_junk_name("Rádio Difusora"));
    }

    #[test]
    fn rebuilds_internal_listen_urls() {
        assert_eq!(
            public_url("http", "kcm.fm:8000", "http://127.0.0.1:8000/02"),
            "http://kcm.fm:8000/02"
        );
        assert_eq!(
            public_url("http", "host:8000", "http://public.host:8000/x"),
            "http://public.host:8000/x"
        );
        assert!(is_internal_host("10.0.6.14"));
        assert!(is_internal_host("172.20.0.1"));
        assert!(!is_internal_host("kcm.fm"));
    }
}

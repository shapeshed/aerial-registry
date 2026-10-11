//! Internet-wide discovery of Icecast servers via scan services (Netlas,
//! Shodan, Censys) or the public Xiph Yellow Pages feed.
//!
//! Invoked as `aerial-registry icecast-scan [yp|netlas|file|shodan|censys]`
//! (default `yp`), or `aerial-registry icecast-scan file [path]` to re-enumerate
//! a saved host list. This is a research/measurement tool: it finds Icecast
//! servers, enumerates the mounts each one exposes, and reports the yield.
//!
//! Every discovery source appends the hosts it found to a reusable seed file
//! (`icecast-hosts.txt` by default), so a limited API quota only has to be
//! spent once — later runs re-enumerate from the file. Credentials come from
//! the layered config (`netlas.api_key` / `shodan.api_key` / `censys.pat`).

use std::collections::HashSet;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tracing::{error, info};

use crate::http::{Client, url_with_query};

const YP_URL: &str = "https://dir.xiph.org/yp.xml";
const SHODAN_SEARCH: &str = "https://api.shodan.io/shodan/host/search";
const CENSYS_SEARCH: &str = "https://api.platform.censys.io/v3/global/search/query";
const NETLAS_DOWNLOAD: &str = "https://app.netlas.io/api/responses/download/";
const SCAN_CONCURRENCY: usize = 64;
const DEFAULT_HOSTS_PATH: &str = "icecast-hosts.txt";

const SHODAN_QUERIES: &[&str] = &[
    "http.title:\"Icecast Streaming Media Server\"",
    "http.html:\"Icecast2 Status\"",
    "product:Icecast",
];

const CENSYS_QUERIES: &[&str] = &[
    "services.software.product: Icecast",
    "services.http.response.html_title: \"Icecast2 Status\"",
];

const NETLAS_QUERIES: &[&str] = &["http.title:\"Icecast Streaming Media Server\""];

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
}

pub async fn run(client: &Client, source: Option<&str>, arg: Option<&str>) -> anyhow::Result<()> {
    match source.unwrap_or("yp") {
        "netlas" => netlas(client).await,
        "file" | "hosts" => {
            let hosts = load_hosts(arg.unwrap_or(DEFAULT_HOSTS_PATH))?;
            scan_hosts(&hosts, "Hosts").await
        }
        "shodan" => shodan(client).await,
        "censys" => censys(client).await,
        "yp" => yellow_pages().await,
        other => {
            error!(
                source = other,
                "Unknown scan source (use yp|netlas|file|shodan|censys)"
            );
            Ok(())
        }
    }
}

/// Enumerate every host in `hosts`, reporting how many expose Icecast mounts.
async fn scan_hosts(hosts: &[String], label: &str) -> anyhow::Result<()> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("aerial-registry/scan")
        .build()?;

    let host_count = hosts.len();
    let results = stream::iter(hosts.to_vec())
        .map(|host| {
            let http = http.clone();
            async move {
                let sources = enumerate(&http, &host).await;
                (host, sources)
            }
        })
        .buffer_unordered(SCAN_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    let mut responsive = 0usize;
    let mut mounts = 0usize;
    let mut sample: Vec<String> = Vec::new();
    for (host, sources) in results {
        let Some(sources) = sources else { continue };
        responsive += 1;
        mounts += sources.len();
        if sample.len() < 20 {
            for s in sources.iter().take(1) {
                let name = s.server_name.as_deref().unwrap_or("(unnamed)");
                let genre = s.genre.as_deref().unwrap_or("");
                let url = s.listenurl.as_deref().unwrap_or("");
                sample.push(format!("  {name:<32} {genre:<14} {url}"));
            }
            let _ = host;
        }
    }

    println!("{label}: {host_count} hosts, {responsive} responded, {mounts} mounts");
    if !sample.is_empty() {
        println!("sample:");
        for line in &sample {
            println!("{line}");
        }
    }
    Ok(())
}

/// Read a seed host list: one `host` or `host:port` per line; blank lines and
/// `#` comments are ignored.
fn load_hosts(path: &str) -> anyhow::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read host list {path}: {e}"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect())
}

/// Merge `hosts` into the seed file at `path` (union with what is already
/// there, sorted), returning how many were newly added. This is what makes the
/// discovered hosts reusable: a limited API quota is spent once, and later runs
/// re-enumerate the accumulated list via `icecast-scan file`.
fn save_hosts(path: &str, hosts: &[String]) -> anyhow::Result<usize> {
    let mut all: HashSet<String> = load_hosts(path).unwrap_or_default().into_iter().collect();
    let before = all.len();
    all.extend(hosts.iter().cloned());
    let mut sorted: Vec<String> = all.into_iter().collect();
    sorted.sort();
    let body = sorted.join("\n") + "\n";
    std::fs::write(path, body)
        .map_err(|e| anyhow::anyhow!("cannot write host list {path}: {e}"))?;
    Ok(sorted.len() - before)
}

/// Fetch the public Xiph Yellow Pages directory and enumerate every listed host.
async fn yellow_pages() -> anyhow::Result<()> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("aerial-registry/scan")
        .build()?;

    let xml = http.get(YP_URL).send().await?.text().await?;
    let hosts = yp_hosts(&xml);
    info!(hosts = hosts.len(), "Yellow Pages hosts parsed");
    scan_hosts(&hosts, "Yellow Pages").await
}

/// Extract the unique hostnames from the Yellow Pages XML. The feed is a flat
/// list of `<entry>` elements; pull `<listen_url>` from each and keep its host.
fn yp_hosts(xml: &str) -> Vec<String> {
    let mut hosts = HashSet::new();
    for entry in xml.split("<entry>").skip(1) {
        let Some(start) = entry.find("<listen_url>") else {
            continue;
        };
        let after = &entry[start + "<listen_url>".len()..];
        let Some(end) = after.find("</listen_url>") else {
            continue;
        };
        let url = after[..end].trim();
        if !url.starts_with("http") {
            continue;
        }
        if let Ok(parsed) = reqwest::Url::parse(url)
            && let Some(host) = parsed.host_str()
        {
            hosts.insert(host.to_ascii_lowercase());
        }
    }
    let mut hosts: Vec<String> = hosts.into_iter().collect();
    hosts.sort();
    hosts
}

/// Ask an Icecast server for its JSON stats, which lists every mount.
async fn enumerate(http: &reqwest::Client, host: &str) -> Option<Vec<Source>> {
    for scheme in ["https", "http"] {
        let url = format!("{scheme}://{host}/status-json.xsl");
        let Ok(response) = http.get(&url).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(body) = response.json::<IceStats>().await else {
            continue;
        };
        let sources = match body.icestats?.source? {
            SourceSet::One(source) => vec![source],
            SourceSet::Many(sources) => sources,
        };
        return Some(sources);
    }
    None
}

#[derive(Deserialize)]
struct NetlasItem {
    data: NetlasData,
}

#[derive(Deserialize)]
struct NetlasData {
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

/// The `host:port` to probe for a Netlas response, omitting default ports.
fn netlas_netloc(item: &NetlasItem) -> Option<String> {
    let ip = item.data.ip.as_deref().filter(|ip| !ip.is_empty())?;
    match item.data.port {
        Some(80) | Some(443) | None => Some(ip.to_string()),
        Some(port) => Some(format!("{ip}:{port}")),
    }
}

/// Search Netlas for Icecast hosts, save them to the reusable seed file, then
/// enumerate them. `netlas.size` bounds the responses downloaded per query.
async fn netlas(client: &Client) -> anyhow::Result<()> {
    let config = crate::config::get();
    let Some(key) = config.netlas.api_key.as_deref().filter(|s| !s.is_empty()) else {
        error!("No Netlas key configured — set `netlas.api_key` or AERIAL__NETLAS__API_KEY");
        return Ok(());
    };
    let size = config.netlas.size.unwrap_or(200);
    let path = config
        .netlas
        .hosts_path
        .as_deref()
        .unwrap_or(DEFAULT_HOSTS_PATH);

    let mut hosts: Vec<String> = Vec::new();
    for query in NETLAS_QUERIES {
        info!(query, size, "Netlas search");
        let body = serde_json::json!({
            "q": query,
            "size": size,
            "fields": ["ip", "port", "protocol", "uri", "host"],
            "source_type": "include",
        });
        let request = client
            .post(NETLAS_DOWNLOAD)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {key}"))
            .json(&body);
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                if !status.is_success() {
                    error!(query, %status, body = %&text[..text.len().min(200)], "Netlas search failed");
                    continue;
                }
                let items: Vec<NetlasItem> = serde_json::from_str(&text).unwrap_or_default();
                info!(query, items = items.len(), "Netlas responses");
                hosts.extend(items.iter().filter_map(netlas_netloc));
            }
            Err(error) => error!(query, %error, "Netlas request failed"),
        }
    }

    hosts.sort();
    hosts.dedup();
    let added = save_hosts(path, &hosts)?;
    info!(hosts = hosts.len(), added, path, "Netlas hosts saved");
    scan_hosts(&hosts, "Netlas").await
}

async fn shodan(client: &Client) -> anyhow::Result<()> {
    let config = crate::config::get();
    let Some(key) = config.shodan.api_key.as_deref().filter(|s| !s.is_empty()) else {
        error!("No Shodan key configured — set `shodan.api_key` or AERIAL__SHODAN__API_KEY");
        return Ok(());
    };

    for query in SHODAN_QUERIES {
        info!(query, "Shodan search");
        let url = url_with_query(
            SHODAN_SEARCH,
            &[("key", key), ("query", query), ("page", "1")],
        );
        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                if status.is_success() {
                    println!("--- {query} ---\n{}", &text[..text.len().min(1500)]);
                } else {
                    error!(query, %status, body = %&text[..text.len().min(200)], "Shodan search failed");
                }
            }
            Err(error) => error!(query, %error, "Shodan request failed"),
        }
    }
    Ok(())
}

async fn censys(client: &Client) -> anyhow::Result<()> {
    let config = crate::config::get();
    let Some(pat) = config.censys.pat.as_deref().filter(|s| !s.is_empty()) else {
        error!("No Censys token configured — set `censys.pat` or AERIAL__CENSYS__PAT");
        return Ok(());
    };
    let org_id = config.censys.org_id.as_deref().filter(|s| !s.is_empty());

    for query in CENSYS_QUERIES {
        info!(query, "Censys search");
        let body = serde_json::json!({ "query": query, "page_size": 100 });
        let mut request = client
            .post(CENSYS_SEARCH)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {pat}"))
            .json(&body);
        if let Some(org_id) = org_id {
            request = request.header("X-Organization-ID", org_id);
        }
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                if status.is_success() {
                    println!("--- {query} ---\n{}", &text[..text.len().min(1500)]);
                } else {
                    error!(query, %status, body = %&text[..text.len().min(200)], "Censys search failed");
                }
            }
            Err(error) => error!(query, %error, "Censys request failed"),
        }
    }
    Ok(())
}

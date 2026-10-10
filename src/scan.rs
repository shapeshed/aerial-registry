//! Internet-wide discovery of Icecast servers via scan services (Shodan,
//! Censys) or the public Xiph Yellow Pages feed.
//!
//! Invoked as `aerial-registry icecast-scan [yp|shodan|censys]` (default `yp`).
//! This is a research/measurement tool: it finds Icecast servers, enumerates
//! the mounts each one exposes, and reports the yield. Credentials come from
//! the layered config (`shodan.api_key` / `censys.pat`).

use std::collections::HashSet;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tracing::{error, info};

use crate::http::{Client, url_with_query};

const YP_URL: &str = "https://dir.xiph.org/yp.xml";
const SHODAN_SEARCH: &str = "https://api.shodan.io/shodan/host/search";
const CENSYS_SEARCH: &str = "https://api.platform.censys.io/v3/global/search/query";
const SCAN_CONCURRENCY: usize = 64;

const SHODAN_QUERIES: &[&str] = &[
    "http.title:\"Icecast Streaming Media Server\"",
    "http.html:\"Icecast2 Status\"",
    "product:Icecast",
];

const CENSYS_QUERIES: &[&str] = &[
    "services.software.product: Icecast",
    "services.http.response.html_title: \"Icecast2 Status\"",
];

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

pub async fn run(client: &Client, source: Option<&str>) -> anyhow::Result<()> {
    match source.unwrap_or("yp") {
        "shodan" => shodan(client).await,
        "censys" => censys(client).await,
        "yp" => yellow_pages().await,
        other => {
            error!(source = other, "Unknown scan source (use yp|shodan|censys)");
            Ok(())
        }
    }
}

/// Fetch the public Xiph Yellow Pages directory, enumerate every listed host,
/// and report how many expose Icecast mounts.
async fn yellow_pages() -> anyhow::Result<()> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("aerial-registry/scan")
        .build()?;

    let xml = http.get(YP_URL).send().await?.text().await?;
    let hosts = yp_hosts(&xml);
    let host_count = hosts.len();
    info!(hosts = host_count, "Yellow Pages hosts parsed");

    let results = stream::iter(hosts)
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

    println!(
        "Yellow Pages: {} hosts, {} responded, {} mounts",
        host_count, responsive, mounts
    );
    if !sample.is_empty() {
        println!("sample:");
        for line in &sample {
            println!("{line}");
        }
    }
    Ok(())
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

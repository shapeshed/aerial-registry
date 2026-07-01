use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const DIRETTE_URL: &str = "https://www.raiplaysound.it/dirette.json";
const RAIPLAYSOUND_BASE: &str = "https://www.raiplaysound.it";
const RELINKER_URL: &str = "https://mediapolis.rai.it/relinker/relinkerServlet.htm";

#[derive(Deserialize)]
struct DiretteResponse {
    contents: Vec<Diretta>,
}

#[derive(Deserialize)]
struct Diretta {
    uniquename: String,
    title: String,
    audio: Option<Audio>,
    channel: Option<Channel>,
}

#[derive(Deserialize)]
struct Audio {
    url: String,
}

#[derive(Deserialize)]
struct Channel {
    logo: Option<String>,
}

/// Extracts the `cont` content ID from a relinker URL such as
/// `https://mediapolis.rai.it/relinker/relinkerServlet.htm?cont=162834`.
fn relinker_cont_id(url: &str) -> Option<&str> {
    url.split("cont=").nth(1)?.split('&').next()
}

/// Resolves a relinker content ID to its playable HLS URL. The relinker signs a
/// fresh Akamai/MainStreaming token on every call, so this must run at discovery
/// time rather than being resolved once and reused across builds.
async fn resolve_stream(client: &Client, cont_id: &str) -> Option<String> {
    let resp = client
        .get(RELINKER_URL)
        .query(&[("cont", cont_id), ("output", "45")])
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    let tag = "<url type=\"content\">";
    let start = text.find(tag)? + tag.len();
    let end = text[start..].find("</url>")? + start;
    let stream_url = text[start..end].trim();
    (!stream_url.is_empty()).then(|| stream_url.to_string())
}

/// Most RAI live channels are Italian; the joint-venture San Marino channel is not.
fn country_for(title: &str) -> (&'static str, &'static str) {
    if title.eq_ignore_ascii_case("radio san marino") {
        ("San Marino", "SM")
    } else {
        ("Italy", "IT")
    }
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(DIRETTE_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "rai", "Failed to fetch dirette: {e}");
            return vec![];
        }
    };

    let body: DiretteResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "rai", "Failed to parse dirette response: {e}");
            return vec![];
        }
    };

    let fetches: Vec<_> = body
        .contents
        .into_iter()
        .filter_map(|item| {
            let cont_id = relinker_cont_id(&item.audio?.url)?.to_string();
            // The "-transparent" logo is a white-only wordmark meant to sit on the
            // channel's own brand-colour background; the plain variant at the same
            // path carries the actual brand colour and renders correctly anywhere.
            let logo_url = item
                .channel
                .and_then(|c| c.logo)
                .map(|logo| logo.replace("-transparent.png", ".png"))
                .map(|logo| format!("{RAIPLAYSOUND_BASE}{logo}"));
            Some((item.uniquename, item.title, logo_url, cont_id))
        })
        .map(|(uniquename, title, logo_url, cont_id)| {
            let client = client.clone();
            async move {
                let stream_url = resolve_stream(&client, &cont_id).await;
                (uniquename, title, logo_url, stream_url)
            }
        })
        .collect();

    let results = join_all(fetches).await;

    let mut stations = Vec::new();
    for (uniquename, title, logo_url, stream_url) in results {
        let Some(stream_url) = stream_url else {
            warn!(provider = "rai", name = %title, "No stream resolved — skipping");
            continue;
        };
        let (country, country_code) = country_for(&title);
        debug!(provider = "rai", name = %title, %stream_url, "Discovered station");
        stations.push(Station {
            name: title,
            stream_url,
            logo_url,
            country: Some(country.into()),
            country_code: Some(country_code.into()),
            tags: vec![],
            description: None,
            provider: "rai".into(),
            provider_id: Some(uniquename),
            trusted: true,
        });
    }

    tracing::info!(provider = "rai", count = stations.len(), "Discovery complete");
    stations
}

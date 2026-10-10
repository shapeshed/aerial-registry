use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Semaphore;
use tracing::{debug, error, warn};

use crate::http::Client;
use crate::station::Station;

/// Global Player's data is served by its Next.js app. The public BFF the
/// provider formerly used (`bff-web-guacamole.musicradio.com/stations/`) now
/// returns 404 for every path.
///
/// The full station list — including every regional variant — is published in
/// the radio sitemap, and each station's `/live/{brand}/{station}` page carries
/// its playback URLs. The build id changes on every deploy, so it is read from
/// the homepage first.
const HOMEPAGE_URL: &str = "https://www.globalplayer.com/";
const SITEMAP_URL: &str = "https://www.globalplayer.com/sitemaps/sitemap_radio.xml";
const BASE: &str = "https://www.globalplayer.com";

/// The per-station endpoint intermittently returns 5xx under load, so requests
/// are bounded (the retry/backoff itself lives in `crate::http`).
const MAX_CONCURRENCY: usize = 16;

#[derive(Deserialize)]
struct NextData<T> {
    #[serde(rename = "pageProps", default)]
    page_props: T,
}

#[derive(Deserialize, Default)]
struct StationProps {
    #[serde(default)]
    station: StationInfo,
    #[serde(default)]
    playable: Playable,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StationInfo {
    id: Option<String>,
    gduid: Option<String>,
    name: Option<String>,
    tagline: Option<String>,
    brand_logo: Option<String>,
}

#[derive(Deserialize, Default)]
struct Playable {
    #[serde(default)]
    playback: Vec<Playback>,
}

#[derive(Deserialize, Default)]
struct Playback {
    url: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
}

async fn fetch_build_id(client: &Client) -> Option<String> {
    let html = client
        .get(HOMEPAGE_URL)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let key = "\"buildId\":\"";
    let start = html.find(key)? + key.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_owned())
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let Some(build_id) = fetch_build_id(client).await else {
        error!(provider = "global", "Could not fetch build ID");
        return vec![];
    };

    let sitemap = match client.get(SITEMAP_URL).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(provider = "global", "Sitemap request failed: {e}");
                return vec![];
            }
        },
        Err(e) => {
            error!(provider = "global", "Sitemap request failed: {e}");
            return vec![];
        }
    };

    let paths = sitemap_paths(&sitemap);
    if paths.is_empty() {
        error!(
            provider = "global",
            "No stations found in the radio sitemap"
        );
        return vec![];
    }
    debug!(provider = "global", count = paths.len(), "Sitemap stations");

    // Each station's own page carries its name and playback URLs. The endpoint
    // intermittently 5xxs under load, so requests are bounded and retried.
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));
    let fetches: Vec<_> = paths
        .iter()
        .map(|(brand, station)| {
            let client = client.clone();
            let build_id = build_id.clone();
            let semaphore = semaphore.clone();
            let brand = brand.clone();
            let station = station.clone();
            async move {
                let _permit = semaphore.acquire().await.ok()?;
                let url = format!("{BASE}/_next/data/{build_id}/live/{brand}/{station}.json");
                let props = match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => resp
                        .json::<NextData<StationProps>>()
                        .await
                        .ok()
                        .map(|data| data.page_props),
                    _ => None,
                };
                if props.is_none() {
                    warn!(provider = "global", %brand, %station, "No station data");
                }
                props
            }
        })
        .collect();

    let mut stations = Vec::new();
    for props in futures::future::join_all(fetches)
        .await
        .into_iter()
        .flatten()
    {
        let Some(name) = props.station.name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let Some(stream_url) = pick_stream(&props.playable) else {
            warn!(provider = "global", %name, "Skipped — no public stream URL");
            continue;
        };
        debug!(provider = "global", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url: props.station.brand_logo,
            country: Some("United Kingdom".into()),
            country_code: Some("GB".into()),
            tags: vec![],
            description: props.station.tagline.filter(|t| !t.is_empty()),
            provider: "global".into(),
            provider_id: props
                .station
                .gduid
                .filter(|v| !v.is_empty())
                .or_else(|| props.station.id.filter(|v| !v.is_empty())),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "global",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Extract `(brand, station)` pairs from the radio sitemap's
/// `/live/{brand}/{station}/` URLs.
fn sitemap_paths(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for chunk in xml.split("<loc>").skip(1) {
        let Some(end) = chunk.find("</loc>") else {
            continue;
        };
        let url = chunk[..end].trim();
        let Some(rest) = url.split("/live/").nth(1) else {
            continue;
        };
        let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            out.push((parts[0].to_owned(), parts[1].to_owned()));
        }
    }
    out
}

/// Pick a publicly playable URL: the subscriber (`-plus`) entries carry
/// `auth.license`/`AdFree` flags and the HD entry carries `auth.HDAuth`; the
/// ad-supported entry is the plain public stream.
fn pick_stream(playable: &Playable) -> Option<String> {
    playable
        .playback
        .iter()
        .find(|p| {
            p.url.as_deref().is_some_and(|u| u.starts_with("http"))
                && !p
                    .flags
                    .iter()
                    .any(|f| matches!(f.as_str(), "auth.license" | "auth.HDAuth" | "AdFree"))
        })
        .and_then(|p| p.url.clone())
}

#[cfg(test)]
mod tests {
    use super::{Playable, pick_stream, sitemap_paths};

    #[test]
    fn parses_regional_and_national_paths_from_sitemap() {
        let xml = r#"<urlset>
            <url><loc>https://www.globalplayer.com/live/capital/uk/</loc></url>
            <url><loc>https://www.globalplayer.com/live/capital/teesside/</loc></url>
            <url><loc>https://www.globalplayer.com/somewhere/else/</loc></url>
        </urlset>"#;
        let paths = sitemap_paths(xml);
        assert_eq!(
            paths,
            vec![
                ("capital".to_string(), "uk".to_string()),
                ("capital".to_string(), "teesside".to_string())
            ]
        );
    }

    #[test]
    fn picks_the_public_stream_not_the_subscriber_one() {
        let playable: Playable = serde_json::from_str(
            r#"{"playback":[
                {"url":"https://hls.thisisdax.com/hls/CapitalXTRA-plus/master.m3u8","flags":["format.hls","auth.license","AdFree"]},
                {"url":"https://media-ssl.musicradio.com/CapitalXTRANationalHD","flags":["hd","auth.HDAuth"]},
                {"url":"https://media-ssl.musicradio.com/CapitalXTRANational","flags":["format.icecast","GlobalAdSupported"]}
            ]}"#,
        )
        .unwrap();
        assert_eq!(
            pick_stream(&playable).as_deref(),
            Some("https://media-ssl.musicradio.com/CapitalXTRANational")
        );
    }
}

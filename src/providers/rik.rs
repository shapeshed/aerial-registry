use futures::future::join_all;
use reqwest::Client;
use tracing::{debug, error, warn};

use crate::station::Station;

/// CyBC/ΡΙΚ's radio site has one live page per channel carrying both the
/// HLS stream (cloudskep CDN) and the channel logo. The logo host embeds an
/// Aldryn deployment hash that can churn on redeploys, so both are resolved
/// from the pages at discovery time.
const LIVE_BASE: &str = "https://radio.rik.cy/live-radio";
const COUNTRY: &str = "Cyprus";
const COUNTRY_CODE: &str = "CY";

const STATIONS: &[(&str, &str)] = &[
    // (page slug and provider_id, display name)
    ("rik-1", "ΡΙΚ Πρώτο"),
    ("rik-2", "ΡΙΚ Δεύτερο"),
    ("rik-3", "ΡΙΚ Τρίτο"),
    ("rik-4", "ΡΙΚ Τέταρτο"),
];

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches: Vec<_> = STATIONS
        .iter()
        .map(|(slug, display_name)| {
            let client = client.clone();
            async move {
                let url = format!("{LIVE_BASE}/{slug}/");
                let html = match client.get(&url).send().await {
                    Ok(r) => r.text().await.unwrap_or_default(),
                    Err(e) => {
                        error!(
                            provider = "rik",
                            station = slug,
                            "Failed to fetch live page: {e}"
                        );
                        String::new()
                    }
                };
                let number = slug.rsplit('-').next().unwrap_or_default();
                (
                    *slug,
                    *display_name,
                    extract_stream(&html),
                    extract_logo(&html, number),
                )
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (slug, display_name, stream_url, logo_url) in join_all(fetches).await {
        let Some(stream_url) = stream_url else {
            warn!(
                provider = "rik",
                station = slug,
                "No HLS stream on live page — skipping"
            );
            continue;
        };
        if logo_url.is_none() {
            warn!(
                provider = "rik",
                station = slug,
                "No channel logo on live page"
            );
        }
        debug!(provider = "rik", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "rik".into(),
            provider_id: Some(slug.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rik",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

fn extract_stream(html: &str) -> Option<String> {
    let start = html.find("https://")?;
    let mut rest = &html[start..];
    loop {
        let end = rest.find(['"', '\'', ' ', '<'])?;
        let url = &rest[..end];
        if url.ends_with(".m3u8") {
            return Some(url.to_string());
        }
        let next = rest[1..].find("https://")? + 1;
        rest = &rest[next..];
    }
}

/// The pages embed every channel's logo; pick the one whose filename marks
/// this channel (`N._rik-radio-N_logo…png`), preferring the `original`
/// rendition over social-card crops.
fn extract_logo(html: &str, number: &str) -> Option<String> {
    let marker = format!("rik-radio-{number}_logo");
    let mut best: Option<&str> = None;
    let mut rest = html;
    while let Some(pos) = rest.find("https://") {
        let candidate = &rest[pos..];
        let end = candidate.find(['"', '\'', ' ', '<'])?;
        let url = &candidate[..end];
        if url.contains(&marker) && url.ends_with(".png") {
            if url.contains(".original.") {
                return Some(url.to_string());
            }
            best.get_or_insert(url);
        }
        rest = &candidate[8..];
    }
    best.map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{extract_logo, extract_stream};

    const PAGE: &str = r#"
        <img src="https://cybc-live-abc.aldryn-media.com/images/1._rik-radio-1_logo.2e16d0ba.fill-1200x630.png">
        <img src="https://cybc-live-abc.aldryn-media.com/images/1._rik-radio-1_logo.original.png">
        <img src="https://cybc-live-abc.aldryn-media.com/images/2._rik-radio-2_logo_2.original.png">
        <script>var s = "https://r1.cloudskep.com/cybcr/cybc1/playlist.m3u8";</script>
    "#;

    #[test]
    fn extracts_stream_url() {
        assert_eq!(
            extract_stream(PAGE).as_deref(),
            Some("https://r1.cloudskep.com/cybcr/cybc1/playlist.m3u8")
        );
        assert_eq!(extract_stream("<html>none</html>"), None);
    }

    #[test]
    fn extracts_matching_channel_logo() {
        assert_eq!(
            extract_logo(PAGE, "1").as_deref(),
            Some("https://cybc-live-abc.aldryn-media.com/images/1._rik-radio-1_logo.original.png")
        );
        assert_eq!(
            extract_logo(PAGE, "2").as_deref(),
            Some(
                "https://cybc-live-abc.aldryn-media.com/images/2._rik-radio-2_logo_2.original.png"
            )
        );
        assert_eq!(extract_logo(PAGE, "9"), None);
    }
}

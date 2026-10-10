use crate::http::Client;
use futures::future::join_all;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "North Macedonia";
const COUNTRY_CODE: &str = "MK";

/// MRT's player (play.mrt.com.mk) streams each radio channel as HLS from
/// interspace.com with a short-lived signed token (`wmsAuthSign`, 30 minutes).
/// The token is embedded in each live page's inline player config, so the URL
/// must be resolved at every discovery run — as with RAI's relinker — rather
/// than stored statically.
const LIVE_BASE: &str = "https://play.mrt.com.mk/live";

/// (live page slug, display name)
const STATIONS: &[(&str, &str)] = &[
    ("radio1", "Македонско Радио 1"),
    ("radio2", "Македонско Радио 2"),
    ("radio3", "Македонско Радио 3"),
    ("radio-sat", "Македонско Радио Сат"),
];

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches: Vec<_> = STATIONS
        .iter()
        .map(|(slug, display_name)| {
            let client = client.clone();
            async move {
                let url = format!("{LIVE_BASE}/{slug}");
                let html = match client.get(&url).send().await {
                    Ok(resp) => resp.text().await.unwrap_or_default(),
                    Err(e) => {
                        error!(provider = "mrt", slug, "Failed to fetch live page: {e}");
                        String::new()
                    }
                };
                (*slug, *display_name, html)
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (slug, display_name, html) in join_all(fetches).await {
        let Some(stream_url) = extract(&html, "\"type\":\"application/x-mpegURL\",\"src\":\"")
        else {
            warn!(
                provider = "mrt",
                slug, "No signed HLS manifest on live page — skipping"
            );
            continue;
        };
        let logo_url = extract(&html, "\"poster\":\"");
        debug!(provider = "mrt", name = display_name, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "mrt".into(),
            provider_id: Some(slug.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "mrt",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Return the value of the first `needle` occurrence, up to the next `"`.
fn extract(html: &str, needle: &str) -> Option<String> {
    let start = html.find(needle)? + needle.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    value.starts_with("http").then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn extracts_signed_manifest_and_poster() {
        let html = r#"var gxArCurrPlaylist = [[{"type":"application/x-mpegURL","src":"https://vod-c57.interspace.com:443/channel_abr/47/playlist.m3u8?wmsAuthSign=abc","label":"Auto","poster":"https://vod-c57w.interspace.com/t/1/chn/47/poster-ln-1.png"}]];"#;
        assert_eq!(
            extract(html, "\"type\":\"application/x-mpegURL\",\"src\":\"").as_deref(),
            Some("https://vod-c57.interspace.com:443/channel_abr/47/playlist.m3u8?wmsAuthSign=abc")
        );
        assert_eq!(
            extract(html, "\"poster\":\"").as_deref(),
            Some("https://vod-c57w.interspace.com/t/1/chn/47/poster-ln-1.png")
        );
        assert_eq!(extract("<html>no player</html>", "\"poster\":\""), None);
    }
}

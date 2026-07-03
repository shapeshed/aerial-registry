use futures::future::join_all;
use reqwest::Client;
use tracing::{debug, error, warn};

use crate::station::Station;

/// RTVA (Andorra) runs on the hiway.media OTT platform with no public
/// channel API. Each live page's server-rendered markup carries the current
/// HLS manifest; the URL embeds infrastructure assignments (gpu node, outgest
/// UUID) that can reshuffle, so streams are resolved from the pages at
/// discovery time. The manifests serve without tokens.
const LIVE_BASE: &str = "https://www.rtva.ad/en-directe";
const COUNTRY: &str = "Andorra";
const COUNTRY_CODE: &str = "AD";

/// RTVA publishes no per-channel radio artwork: RNA carries the
/// broadcaster's own current web logo, Andorra Música its mark from
/// Wikimedia Commons.
const STATIONS: &[(&str, &str, &str)] = &[
    // (live page slug and provider_id, display name, logo)
    (
        "rna",
        "Ràdio Nacional d'Andorra",
        "https://mediaverse.rtva.hiway.media/image/2025/04/30/6811d43d/Logo-RTVA-web.png",
    ),
    (
        "am",
        "Andorra Música",
        "https://upload.wikimedia.org/wikipedia/commons/9/9a/Andorra_M%C3%BAsica-RTVA_%282013%29.png",
    ),
];

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches: Vec<_> = STATIONS
        .iter()
        .map(|(slug, display_name, logo_url)| {
            let client = client.clone();
            async move {
                let url = format!("{LIVE_BASE}/{slug}");
                let html = match client.get(&url).send().await {
                    Ok(r) => r.text().await.unwrap_or_default(),
                    Err(e) => {
                        error!(
                            provider = "rtva",
                            station = slug,
                            "Failed to fetch live page: {e}"
                        );
                        String::new()
                    }
                };
                (*slug, *display_name, *logo_url, extract_manifest(&html))
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (slug, display_name, logo_url, stream_url) in join_all(fetches).await {
        let Some(stream_url) = stream_url else {
            warn!(
                provider = "rtva",
                station = slug,
                "No HLS manifest on live page — skipping"
            );
            continue;
        };
        debug!(provider = "rtva", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url: Some(logo_url.to_string()),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "rtva".into(),
            provider_id: Some(slug.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rtva",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Pull the first hiway live manifest out of the page, dropping the empty
/// `?t=` token parameter the player template carries (the manifests serve
/// without it).
fn extract_manifest(html: &str) -> Option<String> {
    let start = html.find("https://livesg.")?;
    let candidate = &html[start..];
    let end = candidate.find(['"', '\\', '\''])?;
    let url = &candidate[..end];
    let url = url.split('?').next().unwrap_or(url);
    url.ends_with(".m3u8").then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::extract_manifest;

    #[test]
    fn extracts_manifest_and_strips_token_param() {
        let html = r#"{"src":"https://livesg.rtva.hiway.media/restreamer/rtva_client/gpu-f-c0-7/restreamer/outgest/578e48f3/manifest.m3u8?t="}"#;
        assert_eq!(
            extract_manifest(html).as_deref(),
            Some(
                "https://livesg.rtva.hiway.media/restreamer/rtva_client/gpu-f-c0-7/restreamer/outgest/578e48f3/manifest.m3u8"
            )
        );
        assert_eq!(extract_manifest("<html>no player here</html>"), None);
    }
}

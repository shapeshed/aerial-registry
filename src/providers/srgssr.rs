use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// SRG SSR's integration layer is the open API behind all four Play
/// platforms: per business unit (SRF German, RTS French, RSI Italian, RTR
/// Romansh) a livestream list carries titles and artwork, and a media
/// composition per stream URN resolves the current resources. The
/// progressive MP3 resource is preferred — a stable direct stream.
const IL_BASE: &str = "https://il.srgssr.ch/integrationlayer";
const BUSINESS_UNITS: &[&str] = &["srf", "rts", "rsi", "rtr"];
const COUNTRY: &str = "Switzerland";
const COUNTRY_CODE: &str = "CH";

#[derive(Deserialize)]
struct MediaList {
    #[serde(rename = "mediaList", default)]
    media_list: Vec<Media>,
}

#[derive(Deserialize)]
struct Media {
    urn: String,
    title: String,
    #[serde(rename = "imageUrl")]
    image_url: Option<String>,
}

#[derive(Deserialize)]
struct Composition {
    #[serde(rename = "chapterList", default)]
    chapter_list: Vec<Chapter>,
}

#[derive(Deserialize)]
struct Chapter {
    #[serde(rename = "resourceList", default)]
    resource_list: Vec<Resource>,
}

#[derive(Deserialize)]
struct Resource {
    url: String,
    #[serde(default)]
    streaming: String,
    #[serde(default)]
    encoding: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let lists = join_all(BUSINESS_UNITS.iter().map(|bu| {
        let client = client.clone();
        async move {
            let url = format!("{IL_BASE}/2.0/{bu}/mediaList/audio/livestreams.json");
            match client.get(&url).send().await {
                Ok(r) => match r.json::<MediaList>().await {
                    Ok(l) => l.media_list,
                    Err(e) => {
                        error!(provider = "srgssr", bu, "Failed to parse livestreams: {e}");
                        vec![]
                    }
                },
                Err(e) => {
                    error!(provider = "srgssr", bu, "Failed to fetch livestreams: {e}");
                    vec![]
                }
            }
        }
    }))
    .await;

    let resolutions = join_all(lists.into_iter().flatten().map(|media| {
        let client = client.clone();
        async move {
            let stream = resolve_stream(&client, &media.urn).await;
            (media, stream)
        }
    }))
    .await;

    let mut stations = Vec::new();
    for (media, stream) in resolutions {
        let Some(stream_url) = stream else {
            warn!(provider = "srgssr", urn = %media.urn, "No stream resource — skipping");
            continue;
        };
        let name = clean_title(&media.title);
        let provider_id = slug_from_stream(&stream_url).unwrap_or_else(|| media.urn.clone());
        debug!(provider = "srgssr", name = %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url: media.image_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "srgssr".into(),
            provider_id: Some(provider_id),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "srgssr",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn resolve_stream(client: &Client, urn: &str) -> Option<String> {
    let url = format!("{IL_BASE}/2.1/mediaComposition/byUrn/{urn}.json");
    let composition: Composition = match client.get(&url).send().await {
        Ok(r) => r.json().await.ok()?,
        Err(e) => {
            error!(provider = "srgssr", urn, "Failed to fetch composition: {e}");
            return None;
        }
    };
    let resources: Vec<&Resource> = composition
        .chapter_list
        .iter()
        .flat_map(|c| c.resource_list.iter())
        .collect();
    resources
        .iter()
        .find(|r| r.streaming == "PROGRESSIVE" && r.encoding == "MP3")
        .or_else(|| resources.iter().find(|r| r.streaming == "HLS"))
        .map(|r| r.url.clone())
}

/// Titles arrive decorated per language: "Livestream für Radio SRF 1",
/// "RTS Première en direct", "Rete Uno - live".
fn clean_title(title: &str) -> String {
    let t = title.trim();
    let t = t.strip_prefix("Livestream für ").unwrap_or(t);
    let t = t.strip_prefix("Livestream ").unwrap_or(t);
    let t = t.strip_suffix(" en direct").unwrap_or(t);
    let t = t.strip_suffix(" - live").unwrap_or(t);
    let t = t.strip_suffix(" live").unwrap_or(t);
    t.trim().to_string()
}

/// The progressive URLs carry the channel slug:
/// `https://stream.srg-ssr.ch/srgssr/<slug>/mp3/128`.
fn slug_from_stream(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://stream.srg-ssr.ch/srgssr/")?;
    let slug = rest.split('/').next()?;
    (!slug.is_empty()).then(|| slug.to_string())
}

#[cfg(test)]
mod tests {
    use super::{clean_title, slug_from_stream};

    #[test]
    fn cleans_language_decorated_titles() {
        assert_eq!(clean_title("Livestream für Radio SRF 1"), "Radio SRF 1");
        assert_eq!(clean_title("RTS Première en direct"), "RTS Première");
        assert_eq!(clean_title("Rete Uno - live"), "Rete Uno");
        assert_eq!(clean_title("Rete Due"), "Rete Due");
        assert_eq!(clean_title("Radio RTR"), "Radio RTR");
    }

    #[test]
    fn extracts_slug_from_progressive_url() {
        assert_eq!(
            slug_from_stream("https://stream.srg-ssr.ch/srgssr/srf1/mp3/128").as_deref(),
            Some("srf1")
        );
        assert_eq!(
            slug_from_stream(
                "https://stxt-audiostreaming.akamaized.net/hls/live/2117380/srf1/master.m3u8"
            ),
            None
        );
    }
}

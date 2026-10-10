use reqwest::Client;
use tracing::{debug, error};

use crate::station::Station;

const COUNTRY: &str = "Serbia";
const COUNTRY_CODE: &str = "RS";
const PROVIDER: &str = "rts";

/// RTS's radio player (rts-radio.spectar.tv) reads its channel list from a
/// small, public XML document. Each `<channel>` carries the HLS manifest, a
/// poster image and an availability flag. There are no extra hidden channels:
/// the RTS site's digital spin-offs (Pletenica, Rok, Džuboks, …) are not in
/// this document, and the wider RTSPlaneta API requires a key (401).
const CHANNELS_URL: &str = "https://rts-radio.spectar.tv/channels.xml";

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = match client.get(CHANNELS_URL).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(
                    provider = PROVIDER,
                    url = CHANNELS_URL,
                    "Channel list request failed: {e}"
                );
                return Vec::new();
            }
        },
        Err(e) => {
            error!(
                provider = PROVIDER,
                url = CHANNELS_URL,
                "Channel list request failed: {e}"
            );
            return Vec::new();
        }
    };

    let stations = parse_channels(&body);
    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// The document is a flat list of `<channel>` blocks with one occurrence of each
/// child tag, so a small field extractor is enough — no XML dependency.
fn parse_channels(body: &str) -> Vec<Station> {
    let mut stations = Vec::new();
    for block in body.split("<channel>").skip(1) {
        let block = block.split("</channel>").next().unwrap_or(block);
        // Only channels the broadcaster marks available have a usable manifest.
        if field(block, "available").as_deref() != Some("1") {
            continue;
        }
        let (Some(id), Some(name), Some(url)) = (
            field(block, "id"),
            field(block, "name"),
            field(block, "url"),
        ) else {
            continue;
        };
        if !url.starts_with("http") {
            continue;
        }

        let logo_url = field(block, "poster").or_else(|| field(block, "img"));
        debug!(provider = PROVIDER, name, %url, "Discovered station");
        stations.push(Station {
            name,
            stream_url: url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: PROVIDER.into(),
            provider_id: Some(id),
            trusted: true,
        });
    }
    stations
}

fn field(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    let value = block[start..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_channels;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<response>
    <error status="0"/>
    <channel>
        <id>15918</id>
        <name>Радио Београд 1</name>
        <url>https://rtsradio-live.morescreens.com/RTS_2_001/playlist.m3u8</url>
        <img>https://example.com/thumb.jpg</img>
        <poster>https://example.com/poster.jpg</poster>
        <available>1</available>
    </channel>
    <channel>
        <id>99999</id>
        <name>Unavailable</name>
        <url>https://example.com/dead.m3u8</url>
        <available>0</available>
    </channel>
</response>"#;

    #[test]
    fn parses_available_channels_only() {
        let stations = parse_channels(SAMPLE);
        assert_eq!(stations.len(), 1);
        let s = &stations[0];
        assert_eq!(s.name, "Радио Београд 1");
        assert_eq!(
            s.stream_url,
            "https://rtsradio-live.morescreens.com/RTS_2_001/playlist.m3u8"
        );
        assert_eq!(
            s.logo_url.as_deref(),
            Some("https://example.com/poster.jpg")
        );
        assert_eq!(s.provider_id.as_deref(), Some("15918"));
        assert!(s.trusted);
    }
}

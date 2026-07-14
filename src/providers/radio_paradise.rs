use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const CHANNELS_URL: &str =
    "https://vsh-sdata.radioparadise.com/api/channels?populate=banner&pagination[pageSize]=50";
const IMAGE_BASE: &str = "https://vsh-sdata.radioparadise.com";

/// Radio Paradise's channel metadata API has no stream URL field at all —
/// just name/summary/artwork. The direct stream host follows a stable,
/// long-documented naming convention (confirmed against community-submitted
/// Radio Browser entries for the same channels), keyed on `chan_id` since a
/// channel's slug doesn't always match its display name (Main Mix -> `aac`).
/// "Mellow X" (4) and "My Favorites" (99, a personalised aggregate, not a
/// broadcast) have no known direct stream and are skipped.
const STREAM_SLUGS: &[(i64, &str)] = &[
    (0, "aac-320"),        // Main Mix
    (1, "mellow-320"),     // Mellow Mix
    (2, "rock-320"),       // Rock Mix
    (3, "global-320"),     // Global Mix
    (5, "beyond-320"),     // Beyond
    (42, "serenity-flac"), // Serenity (FLAC only — no bitrate-suffixed variant)
    (945, "kfat-320"),     // KFAT
];

#[derive(Deserialize)]
struct ChannelsResponse {
    data: Vec<ChannelEntry>,
}

#[derive(Deserialize)]
struct ChannelEntry {
    attributes: ChannelAttrs,
}

#[derive(Deserialize)]
struct ChannelAttrs {
    name: String,
    chan_id: i64,
    summary: Option<String>,
    banner: Option<BannerRelation>,
}

#[derive(Deserialize)]
struct BannerRelation {
    data: Option<BannerData>,
}

#[derive(Deserialize)]
struct BannerData {
    attributes: BannerAttrs,
}

#[derive(Deserialize)]
struct BannerAttrs {
    url: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(CHANNELS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "radio-paradise", "Failed to fetch channels: {e}");
            return vec![];
        }
    };

    let body: ChannelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "radio-paradise", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();
    for entry in body.data {
        let attrs = entry.attributes;
        let Some(&(_, slug)) = STREAM_SLUGS.iter().find(|(id, _)| *id == attrs.chan_id) else {
            warn!(
                provider = "radio-paradise",
                channel = %attrs.name,
                chan_id = attrs.chan_id,
                "No known direct stream for this channel — skipping"
            );
            continue;
        };
        let stream_url = format!("https://stream.radioparadise.com/{slug}");

        let logo_url = attrs
            .banner
            .and_then(|b| b.data)
            .map(|d| format!("{IMAGE_BASE}{}", d.attributes.url));

        debug!(provider = "radio-paradise", name = %attrs.name, %stream_url, "Discovered station");
        stations.push(Station {
            name: attrs.name,
            stream_url,
            logo_url,
            country: Some("United States".into()),
            country_code: Some("US".into()),
            tags: vec![],
            description: attrs.summary.filter(|d| !d.is_empty()),
            provider: "radio-paradise".into(),
            provider_id: Some(attrs.chan_id.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "radio-paradise",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_channel_maps_to_its_stream_slug() {
        let (_, slug) = STREAM_SLUGS.iter().find(|(id, _)| *id == 0).unwrap();
        assert_eq!(*slug, "aac-320");
    }

    #[test]
    fn unlisted_channel_has_no_slug() {
        assert!(!STREAM_SLUGS.iter().any(|(id, _)| *id == 99));
        assert!(!STREAM_SLUGS.iter().any(|(id, _)| *id == 4));
    }

    #[test]
    fn channels_response_parses_expected_shape() {
        let body = r#"{
            "data": [
                {
                    "id": 2,
                    "attributes": {
                        "name": "Main Mix",
                        "chan_id": 0,
                        "summary": "An eclectic musical adventure.",
                        "banner": {
                            "data": {
                                "attributes": { "url": "/uploads/main.jpg" }
                            }
                        }
                    }
                },
                {
                    "id": 6,
                    "attributes": {
                        "name": "My Favorites",
                        "chan_id": 99,
                        "summary": null,
                        "banner": { "data": null }
                    }
                }
            ]
        }"#;
        let parsed: ChannelsResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].attributes.chan_id, 0);
        let banner = parsed.data[0]
            .attributes
            .banner
            .as_ref()
            .unwrap()
            .data
            .as_ref()
            .unwrap();
        assert_eq!(banner.attributes.url, "/uploads/main.jpg");
        assert!(
            parsed.data[1]
                .attributes
                .banner
                .as_ref()
                .unwrap()
                .data
                .is_none()
        );
    }
}

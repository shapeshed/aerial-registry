use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// Mediaklikk (MTVA's platform) inlines the radio player's full channel map
/// — id, name, short name and live stream — in the radio page as the
/// `bobapSetup` config. Channel logos live on the same host, keyed by the
/// config's channel id.
const RADIO_PAGE: &str = "https://mediaklikk.hu/radio";
const LOGO_BASE: &str = "https://mediaklikk.hu/iface/channel_logos";
const COUNTRY: &str = "Hungary";
const COUNTRY_CODE: &str = "HU";

#[derive(Deserialize)]
struct Channel {
    name: String,
    #[serde(rename = "shortName")]
    short_name: String,
    #[serde(rename = "primary_live_url")]
    primary_live_url: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let html = match client.get(RADIO_PAGE).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => {
            error!(provider = "mtva", "Failed to fetch radio page: {e}");
            return vec![];
        }
    };
    let Some(channels) = parse_channels(&html) else {
        error!(provider = "mtva", "No bobapSetup channel map in radio page");
        return vec![];
    };

    let mut stations = Vec::new();
    for (id, channel) in channels {
        // Channels without a live stream (Duna World is a TV simulcast
        // listing) are not radio stations.
        if channel.primary_live_url.is_empty() {
            continue;
        }
        if channel.name.trim().is_empty() {
            warn!(provider = "mtva", id = %id, "Channel has no name — skipping");
            continue;
        }
        let name = display_name(&channel.name);
        let stream_url = channel.primary_live_url;
        debug!(provider = "mtva", name = %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url: Some(format!("{LOGO_BASE}/{id}_v.png")),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "mtva".into(),
            provider_id: Some(channel.short_name),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "mtva",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// The config names channels by bare brand ("Kossuth", "Petőfi"); the
/// stations' full names carry Rádió, which also keeps them searchable.
fn display_name(name: &str) -> String {
    let name = name.trim();
    if name.to_lowercase().contains("rádió") {
        name.to_string()
    } else {
        format!("{name} Rádió")
    }
}

/// Extract the `"channels": {...}` object inside the inline `bobapSetup`
/// config by brace matching.
fn parse_channels(html: &str) -> Option<Vec<(String, Channel)>> {
    let setup = html.find("bobapSetup")?;
    let key = html[setup..].find("\"channels\":")? + setup;
    let start = html[key..].find('{')? + key;
    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in html[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let map: std::collections::BTreeMap<String, Channel> =
        serde_json::from_str(&html[start..end?]).ok()?;
    (!map.is_empty()).then(|| map.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{display_name, parse_channels};

    #[test]
    fn parses_channel_map() {
        let html = r#"var bobapSetup = {"DOM":{},"channels":{"6":{"name":"Kossuth","shortName":"mr1","primary_live_url":"https://mr-stream.connectmedia.hu/4736/mr1.mp3","other":{"nested":true}},"29":{"name":"Duna World","shortName":"mr8","primary_live_url":""}}};"#;
        let channels = parse_channels(html).unwrap();
        assert_eq!(channels.len(), 2);
        let (id, _) = &channels[0];
        assert_eq!(id, "29"); // BTreeMap orders keys lexically
        let (_, kossuth) = &channels[1];
        assert_eq!(kossuth.name, "Kossuth");
        assert_eq!(kossuth.short_name, "mr1");
    }

    #[test]
    fn appends_radio_to_bare_brands() {
        assert_eq!(display_name("Kossuth"), "Kossuth Rádió");
        assert_eq!(display_name("Nemzeti Sportrádió"), "Nemzeti Sportrádió");
    }
}

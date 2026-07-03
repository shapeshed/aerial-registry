use futures::future::join_all;
use reqwest::Client;
use tracing::{debug, error, warn};

use crate::station::Station;

/// BHRT's iRadio network hosts six web music channels, each a static player
/// whose config.js declares the Shoutcast stream. There is no station API.
const IRADIO_BASE: &str = "https://iradio.bhrt.ba";
const COUNTRY: &str = "Bosnia and Herzegovina";
const COUNTRY_CODE: &str = "BA";

/// The flagship broadcast station streams from BH Telecom's CDN. The URL is
/// geo-restricted from some networks (HTTP 403) but plays fine in the region
/// — exactly the case the geo-aware liveness policy exists for; as a trusted
/// station it is never probed anyway.
const BH_RADIO_1_STREAM: &str = "https://webtvstream.bhtelecom.ba/bh_radio1.m3u8";
const BH_RADIO_1_LOGO: &str =
    "https://upload.wikimedia.org/wikipedia/commons/d/d6/Logo_of_BHRT_%281998-%29.svg";

const IRADIO_CHANNELS: &[(&str, &str)] = &[
    // (slug: page, config, logo and provider_id; display name)
    ("art", "BHRT Art Radio"),
    ("dance", "BHRT Dance Radio"),
    ("evergreen", "BHRT Evergreen Radio"),
    ("nas", "BHRT Naš Radio"),
    ("sevdah", "BHRT Sevdah Radio"),
    ("jazz", "BHRT Jazz Radio"),
];

pub async fn discover(client: &Client) -> Vec<Station> {
    let mut stations = vec![Station {
        name: "BH Radio 1".to_string(),
        stream_url: BH_RADIO_1_STREAM.to_string(),
        logo_url: Some(BH_RADIO_1_LOGO.to_string()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: "bhrt".into(),
        provider_id: Some("bhradio1".to_string()),
        trusted: true,
    }];

    let fetches: Vec<_> = IRADIO_CHANNELS
        .iter()
        .map(|(slug, display_name)| {
            let client = client.clone();
            async move {
                let url = format!("{IRADIO_BASE}/{slug}/config.js");
                let config = match client.get(&url).send().await {
                    Ok(r) => r.text().await.unwrap_or_default(),
                    Err(e) => {
                        error!(
                            provider = "bhrt",
                            station = slug,
                            "Failed to fetch config: {e}"
                        );
                        String::new()
                    }
                };
                (*slug, *display_name, config_value(&config, "url_streaming"))
            }
        })
        .collect();

    for (slug, display_name, stream_url) in join_all(fetches).await {
        let Some(stream_url) = stream_url else {
            warn!(
                provider = "bhrt",
                station = slug,
                "No url_streaming in config — skipping"
            );
            continue;
        };
        debug!(provider = "bhrt", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: display_name.to_string(),
            stream_url,
            logo_url: Some(format!("{IRADIO_BASE}/img/{slug}.png")),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "bhrt".into(),
            provider_id: Some(slug.to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "bhrt",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Pull a value out of the player's config.js, e.g.
/// `'url_streaming': 'https://pstnet7.shoutcastnet.com:10074/stream',`
fn config_value(js: &str, key: &str) -> Option<String> {
    let key_pos = js.find(&format!("'{key}'"))?;
    let rest = &js[key_pos..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let open = rest.find('\'')?;
    let rest = &rest[open + 1..];
    let close = rest.find('\'')?;
    let value = rest[..close].trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::config_value;

    #[test]
    fn extracts_stream_url_from_config() {
        let js = r#"
            var settings = {
                'radio_name': 'Art radio',
                'url_streaming': 'https://pstnet7.shoutcastnet.com:10074/stream',
                'streamtype': 'shoutcast',
            };
        "#;
        assert_eq!(
            config_value(js, "url_streaming").as_deref(),
            Some("https://pstnet7.shoutcastnet.com:10074/stream")
        );
        assert_eq!(config_value(js, "radio_name").as_deref(), Some("Art radio"));
        assert_eq!(config_value(js, "missing"), None);
    }
}

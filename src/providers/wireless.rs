use reqwest::Client;
use serde::{Deserialize, Deserializer};
use tracing::{debug, error};

use crate::station::Station;

const STATIONS_URL: &str = "https://talksport.com/play/api/stations";

#[derive(Deserialize)]
struct WirelessStation {
    name: Option<String>,
    streams: Option<WirelessStreams>,
    thumbnail: Option<LogoField>,
    logo: Option<LogoField>,
}

#[derive(Deserialize)]
struct WirelessStreams {
    progressive: Option<String>,
    hls: Option<String>,
}

// The logo field may be an object {"url": "..."} or a bare string.
#[derive(Debug, Clone)]
struct LogoField(Option<String>);

impl<'de> Deserialize<'de> for LogoField {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let url = match v {
            serde_json::Value::String(s) => Some(s).filter(|s| !s.is_empty()),
            serde_json::Value::Object(map) => map
                .get("url")
                .and_then(|u| u.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            _ => None,
        };
        Ok(LogoField(url))
    }
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let resp = match client.get(STATIONS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "wireless", "Failed to fetch stations: {e}");
            return vec![];
        }
    };

    let raw: Vec<WirelessStation> = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "wireless", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let mut stations = Vec::new();

    for s in raw {
        let name = match s.name.filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => continue,
        };
        let stream_url = match s.streams.and_then(|st| {
            st.progressive
                .filter(|u| !u.is_empty())
                .or_else(|| st.hls.filter(|u| !u.is_empty()))
        }) {
            Some(u) => u,
            None => continue,
        };

        let logo_url = s
            .thumbnail
            .and_then(|l| l.0)
            .or_else(|| s.logo.and_then(|l| l.0));

        debug!(provider = "wireless", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url,
            country: Some("United Kingdom".into()),
            country_code: Some("GB".into()),
            tags: vec![],
            description: None,
            provider: "wireless".into(),
            provider_id: None,
            trusted: true,
        });
    }

    tracing::info!(
        provider = "wireless",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

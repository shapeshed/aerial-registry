use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// RTSH's Icecast has no hostname alias — the raw IP is what the official
/// site and the community-voted Radio Browser entries use. The status page
/// is the discovery source: mounts are only emitted while actually live.
const ICECAST_BASE: &str = "http://79.106.48.2:8000";
const COUNTRY: &str = "Albania";
const COUNTRY_CODE: &str = "AL";

/// Logos are static theme assets on the radio site — the same URLs have
/// served unchanged since at least 2020 (Wayback-verified), despite the
/// hash-style filenames.
///
/// The four regional stations (Gjirokastra, Korça, Kukës, Shkodra) stream
/// from separate subdomain sites, not this Icecast — a later addition.
/// Temporary event mounts (e.g. zgjedhje2025) are excluded by not being in
/// this table.
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (icecast mount, provider_id, display name, logo)
    (
        "radiotirana1",
        "tirana1",
        "Radio Tirana 1",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/d4036e3df81c45e7b7de578c7b0913f0.svg",
    ),
    (
        "radiotirana2",
        "tirana2",
        "Radio Tirana 2",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/b029014ef25441c29c67b69bfc6753b3.svg",
    ),
    (
        "radiotirana3",
        "tirana3",
        "Radio Tirana 3",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/d9658841fa09444bb6fcd14a0f834a80.svg",
    ),
    (
        "radiotiranafemije",
        "femije",
        "Radio Tirana Fëmijë",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/8d45dc2a20694acfab125446f6f7fddc.svg",
    ),
    (
        "radiotiranajazz",
        "jazz",
        "Radio Tirana Jazz",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/37ab4cd722214568aeff088c3c697fe2.svg",
    ),
    (
        "radiotiranaklasik1",
        "klasik",
        "Radio Tirana Klasik",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/c79cf8bba329468f9e376b6ef9cf9b2d.svg",
    ),
    (
        "rti",
        "rti",
        "Radio Tirana International",
        "https://rtsh.al/radio/wp-content/themes/rtsh-radio/assets/img/3bc9e397415c40d8b575f995387304f4.svg",
    ),
];

#[derive(Deserialize)]
struct IceStatus {
    icestats: IceStats,
}

#[derive(Deserialize)]
struct IceStats {
    #[serde(default)]
    source: Sources,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum Sources {
    #[default]
    None,
    One(IceSource),
    Many(Vec<IceSource>),
}

#[derive(Deserialize)]
struct IceSource {
    listenurl: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let live_mounts = match fetch_mounts(client).await {
        Some(m) => m,
        None => return vec![],
    };

    let mut stations = Vec::new();
    for (mount, id, display_name, logo_url) in STATIONS {
        if !live_mounts.iter().any(|m| m == mount) {
            warn!(
                provider = "rtsh",
                station = mount,
                "Mount not live — skipping"
            );
            continue;
        }
        let stream_url = format!("{ICECAST_BASE}/{mount}");
        debug!(provider = "rtsh", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: (*display_name).to_string(),
            stream_url,
            logo_url: Some((*logo_url).to_string()),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "rtsh".into(),
            provider_id: Some((*id).to_string()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "rtsh",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn fetch_mounts(client: &Client) -> Option<Vec<String>> {
    let resp = match client
        .get(format!("{ICECAST_BASE}/status-json.xsl"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "rtsh", "Failed to fetch Icecast status: {e}");
            return None;
        }
    };
    let status: IceStatus = match resp.json().await {
        Ok(s) => s,
        Err(e) => {
            error!(provider = "rtsh", "Failed to parse Icecast status: {e}");
            return None;
        }
    };
    let sources = match status.icestats.source {
        Sources::None => vec![],
        Sources::One(s) => vec![s],
        Sources::Many(v) => v,
    };
    Some(
        sources
            .iter()
            .filter_map(|s| s.listenurl.rsplit('/').next().map(str::to_string))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{IceStatus, Sources};

    #[test]
    fn status_parses_single_and_multiple_sources() {
        let many: IceStatus = serde_json::from_str(
            r#"{"icestats":{"source":[{"listenurl":"http://localhost:8000/radiotirana1"},{"listenurl":"http://localhost:8000/rti"}]}}"#,
        )
        .unwrap();
        assert!(matches!(many.icestats.source, Sources::Many(ref v) if v.len() == 2));

        // Icecast emits a bare object when only one mount is live.
        let one: IceStatus = serde_json::from_str(
            r#"{"icestats":{"source":{"listenurl":"http://localhost:8000/radiotirana1"}}}"#,
        )
        .unwrap();
        assert!(matches!(one.icestats.source, Sources::One(_)));

        let none: IceStatus = serde_json::from_str(r#"{"icestats":{}}"#).unwrap();
        assert!(matches!(none.icestats.source, Sources::None));
    }
}

use std::collections::HashMap;

use crate::http::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const COUNTRY: &str = "Romania";
const COUNTRY_CODE: &str = "RO";
const PROVIDER: &str = "radio-romania";

/// SRR's Icecast host serves the broadcaster's own status document, which
/// enumerates every live mount — a genuine discovery endpoint. The mounts are
/// also reachable over HTTPS on :8443, which is what we store (the status
/// document advertises cleartext :8008 URLs).
const STATUS_URL: &str = "https://stream4.srr.ro:8443/status-json.xsl";
const STREAM_BASE: &str = "https://stream4.srr.ro:8443";
const LOGO_BASE: &str = "https://www.romania-actualitati.ro/templates/default/images";

/// Per-mount artwork. SRR publishes one SVG per service; the mount slug does
/// not map mechanically onto the file name, so the mapping is explicit. Mounts
/// with no published mark (Antena Sibiului, Radio Sighet, the Junior channels)
/// return `None` and fall back to enrichment.
fn logo_for(mount: &str) -> Option<String> {
    let file = match mount {
        "romania-actualitati" => "logo-actualitati.svg",
        "romania-cultural" => "logo-cultural.svg",
        "romania-muzical" => "logo-muzical.svg",
        "antena-satelor" => "logo-antenasatelor.svg",
        "bucuresti-fm" => "logo-bucurestifm.svg",
        "radio-cluj" => "logo-cluj.svg",
        "kolozsvari-radio" => "logo-cluj-maghiara-1.svg",
        "radio-brasov" => "logo-brasov.svg",
        "radio-constanta-am" => "logo-constanta-am.svg",
        "radio-constanta-fm" => "logo-constanta-fm.svg",
        "radio-constanta-flac" => "logo-constanta-fm.svg",
        "radio-constanta-folclor" => "logo-constanta-folclor.svg",
        "radio-convietuiri" => "logo-convietuiri2.svg",
        "radio-iasi-am" | "radio-iasi-fm" => "logo-iasi.svg",
        "radio-oltenia" => "logo-olteniacraiova.svg",
        "radio-resita" | "radio-resita2" => "logo-resita.svg",
        "radio-ro-regio-100ro" => "logo-regional.svg",
        "radio-tezaur" => "logo-tezaur.svg",
        "radio-tgmures-fm" | "radio-tgmures-multi" => "logo-tgmures.svg",
        "radio-tgmures-mag" => "logo-tgmures-maghiara-1.svg",
        "romania-rri1" => "logo-international1.svg",
        "romania-rri2" => "logo-international2.svg",
        "romania-rri3" => "logo-international3.svg",
        "arad-fm" => "stations/logo-arad-fm.svg",
        "timisoara-am" => "stations/logo-timisoara-am.svg",
        "timisoara-fm" => "stations/logo-timisoara-fm.svg",
        "fresh_rv" => "logo-rv-fresh.svg",
        "gold_rv" => "logo-rv-gold.svg",
        "nostalgia_rv" => "logo-rv-nostalgia.svg",
        _ => return None,
    };
    Some(format!("{LOGO_BASE}/{file}"))
}

#[derive(Deserialize)]
struct IceStatus {
    icestats: IceStats,
}

#[derive(Deserialize)]
struct IceStats {
    source: Option<Sources>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Sources {
    One(IceSource),
    Many(Vec<IceSource>),
}

#[derive(Deserialize)]
struct IceSource {
    listenurl: Option<String>,
    server_name: Option<String>,
    listeners: Option<u64>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = match client.get(STATUS_URL).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(
                    provider = PROVIDER,
                    url = STATUS_URL,
                    "Status request failed: {e}"
                );
                return Vec::new();
            }
        },
        Err(e) => {
            error!(
                provider = PROVIDER,
                url = STATUS_URL,
                "Status request failed: {e}"
            );
            return Vec::new();
        }
    };

    let stations = match parse_status(&body) {
        Ok(stations) => stations,
        Err(e) => {
            error!(provider = PROVIDER, "Could not parse status document: {e}");
            return Vec::new();
        }
    };

    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

fn parse_status(body: &str) -> serde_json::Result<Vec<Station>> {
    let status: IceStatus = serde_json::from_str(body)?;
    let sources = match status.icestats.source {
        Some(Sources::One(s)) => vec![s],
        Some(Sources::Many(v)) => v,
        None => Vec::new(),
    };

    // SRR sometimes lists the same service on two mounts (e.g. an AAC and an
    // MP3 feed of Radio Reșița). Keep the busiest mount per name so the app
    // doesn't show duplicate stations.
    let mut best: HashMap<String, (u64, Station)> = HashMap::new();
    for source in sources {
        let Some(mount) = source
            .listenurl
            .as_deref()
            .and_then(|url| url.trim_end_matches('/').rsplit('/').next())
            .map(str::trim)
            .filter(|m| !m.is_empty())
        else {
            continue;
        };
        let Some(name) = source
            .server_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
        else {
            continue;
        };

        let listeners = source.listeners.unwrap_or(0);
        let station = Station {
            name: name.clone(),
            stream_url: format!("{STREAM_BASE}/{mount}"),
            logo_url: logo_for(mount),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: PROVIDER.into(),
            provider_id: Some(mount.to_string()),
            trusted: true,
        };

        match best.get(&name) {
            Some((existing, _)) if *existing >= listeners => {}
            _ => {
                best.insert(name, (listeners, station));
            }
        }
    }

    let mut stations: Vec<Station> = best.into_values().map(|(_, s)| s).collect();
    stations.sort_by(|a, b| a.name.cmp(&b.name));
    for station in &stations {
        debug!(provider = PROVIDER, name = %station.name, url = %station.stream_url, "Discovered station");
    }
    Ok(stations)
}

#[cfg(test)]
mod tests {
    use super::{logo_for, parse_status};

    #[test]
    fn parses_single_and_many_sources_and_dedupes_by_name() {
        let body = r#"{"icestats":{"source":[
            {"listenurl":"http://stream4.srr.ro:8008/romania-actualitati","server_name":" Radio Romania Actualitati ","listeners":1988},
            {"listenurl":"http://stream4.srr.ro:8008/radio-resita","server_name":" Radio Resita ","listeners":46},
            {"listenurl":"http://stream4.srr.ro:8008/radio-resita2","server_name":" Radio Resita ","listeners":0}
        ]}}"#;
        let stations = parse_status(body).unwrap();
        assert_eq!(stations.len(), 2);
        let rra = stations
            .iter()
            .find(|s| s.name == "Radio Romania Actualitati")
            .unwrap();
        assert_eq!(
            rra.stream_url,
            "https://stream4.srr.ro:8443/romania-actualitati"
        );
        assert!(rra.trusted);
        // The busier of the two Radio Resita mounts wins.
        let resita = stations.iter().find(|s| s.name == "Radio Resita").unwrap();
        assert_eq!(resita.provider_id.as_deref(), Some("radio-resita"));
    }

    #[test]
    fn maps_known_mounts_to_logos() {
        assert!(
            logo_for("romania-muzical")
                .unwrap()
                .ends_with("/logo-muzical.svg")
        );
        assert!(
            logo_for("timisoara-fm")
                .unwrap()
                .ends_with("/stations/logo-timisoara-fm.svg")
        );
        assert!(logo_for("radio-sighet").is_none());
    }
}

use crate::http::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Venezuela";
const COUNTRY_CODE: &str = "VE";

/// Radio Nacional de Venezuela (RNV) is the Venezuelan state radio service. It
/// publishes only two live streams on its site, both on its `guri.tepuyserver.net`
/// host; the other RNV brands (Musical, Activa) have no broadcaster-published
/// stream URL. The host exposes no station-list endpoint, so the set is pinned
/// explicitly.
const LOGO: &str = "https://rnv.gob.ve/wp-content/uploads/2026/05/Logo-RNV-210.png";

/// (mount and provider_id, display name, stream URL)
const STATIONS: &[(&str, &str, &str)] = &[
    (
        "informativa",
        "RNV Informativa",
        "https://guri.tepuyserver.net/8048/stream",
    ),
    (
        "juvenil",
        "RNV Juvenil",
        "https://guri.tepuyserver.net/8156/stream",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url)| {
            debug!(provider = "rnv", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some(LOGO.to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "rnv".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "rnv",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::STATIONS;

    #[test]
    fn station_identities_are_distinct() {
        let mut ids: Vec<&str> = STATIONS.iter().map(|(id, _, _)| *id).collect();
        let mut urls: Vec<&str> = STATIONS.iter().map(|(_, _, u)| *u).collect();
        ids.sort();
        ids.dedup();
        urls.sort();
        urls.dedup();
        assert_eq!(ids.len(), STATIONS.len());
        assert_eq!(urls.len(), STATIONS.len());
    }
}

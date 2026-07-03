use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Kosovo";
const COUNTRY_CODE: &str = "XK";

/// RTK's website player is region-restricted, but the Shoutcast mounts on
/// Kosovo Telecom's IP serve worldwide (verified live; the community Radio
/// Browser entries carry 800+ votes). There is no hostname and no channel
/// API, so the two stations are static; logos are the broadcaster's own
/// player assets.
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo)
    (
        "radiokosova",
        "Radio Kosova",
        "http://82.114.72.2:8088/;",
        "https://www.rtklive.com/sq/livestream/img/rk1.png",
    ),
    (
        "radiokosova2",
        "Radio Kosova 2",
        "http://82.114.72.2:8098/;",
        "https://www.rtklive.com/sq/livestream/img/rk2.png",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_url)| {
            debug!(provider = "rtk", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "rtk".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "rtk",
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
        let mut ids: Vec<&str> = STATIONS.iter().map(|(id, _, _, _)| *id).collect();
        let mut streams: Vec<&str> = STATIONS.iter().map(|(_, _, s, _)| *s).collect();
        ids.sort();
        ids.dedup();
        streams.sort();
        streams.dedup();
        assert_eq!(ids.len(), STATIONS.len());
        assert_eq!(streams.len(), STATIONS.len());
    }
}

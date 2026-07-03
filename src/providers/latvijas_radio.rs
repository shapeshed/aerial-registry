use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Latvia";
const COUNTRY_CODE: &str = "LV";
const LOGO_BASE: &str = "https://latvijasradio.lsm.lv/public/assets/design/channels";

/// The player's "other formats" links unwrap to direct Icecast streams on
/// per-channel hosts (`lrNmp0.latvijasradio.lv`), stable for years. LR5
/// (pieci.lv) is absent: it streams only through a web player with no
/// direct endpoint anywhere. Logos are the player's own channel assets
/// (LR2's mark is the generic file the site itself uses for it).
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo path)
    (
        "lr1",
        "Latvijas Radio 1",
        "http://lr1mp0.latvijasradio.lv:8004/",
        "1/lr1_logo.png",
    ),
    (
        "lr2",
        "Latvijas Radio 2",
        "http://lr2mp0.latvijasradio.lv:8004/",
        "2/lr_logo.png",
    ),
    (
        "lr3",
        "Latvijas Radio 3 Klasika",
        "http://lr3mp0.latvijasradio.lv:8004/",
        "3/lr3_logo.png",
    ),
    (
        "lr4",
        "Latvijas Radio 4",
        "http://lr4mp0.latvijasradio.lv:8004/",
        "4/lr4_logo.png",
    ),
    (
        "naba",
        "Radio Naba",
        "http://nabamp0.latvijasradio.lv:8008/",
        "6/lr6_logo.png",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_path)| {
            debug!(provider = "latvijas-radio", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some(format!("{LOGO_BASE}/{logo_path}")),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "latvijas-radio".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "latvijas-radio",
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

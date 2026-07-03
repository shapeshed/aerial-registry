use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Latvia";
const COUNTRY_CODE: &str = "LV";

/// The lrNmp0 hostnames all resolve to one Icecast whose root path serves a
/// single default mount — stations 1–4 played the same stream. The real
/// per-channel streams are the new player's Wowza HLS mounts (lr1a/lr2a/
/// lr3a/naba, each verified distinct); LR4 is absent from that player and
/// uses its long-standing direct Icecast endpoint (1,700+ Radio Browser
/// votes). Square channel logos come from the broadcaster's CDN; LR4 keeps
/// the lsm.lv player asset (no square exists for it).
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo)
    (
        "lr1",
        "Latvijas Radio 1",
        "https://muste.latvijasradio.lv/shoutcast/mp4:lr1a.stream/playlist.m3u8",
        "https://cdn.latvijasradio.lv/media/station/lr1-square-AKLVNE.png",
    ),
    (
        "lr2",
        "Latvijas Radio 2",
        "https://muste.latvijasradio.lv/shoutcast/mp4:lr2a.stream/playlist.m3u8",
        "https://cdn.latvijasradio.lv/media/station/lr2-square-tVPtiL.png",
    ),
    (
        "lr3",
        "Latvijas Radio 3 Klasika",
        "https://muste.latvijasradio.lv/shoutcast/mp4:lr3a.stream/playlist.m3u8",
        "https://cdn.latvijasradio.lv/media/station/lr3-square-uAudnk.png",
    ),
    (
        "lr4",
        "Latvijas Radio 4",
        "http://lr4mp1.latvijasradio.lv:8020/;",
        "https://latvijasradio.lsm.lv/public/assets/design/channels/4/lr4_logo.png",
    ),
    (
        "naba",
        "Radio Naba",
        "https://muste.latvijasradio.lv/shoutcast/mp4:naba.stream/playlist.m3u8",
        "https://cdn.latvijasradio.lv/media/station/lr6-square-DtKFht.png",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_url)| {
            debug!(provider = "latvijas-radio", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some((*logo_url).to_string()),
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

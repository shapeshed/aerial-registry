use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Moldova";
const COUNTRY_CODE: &str = "MD";

/// Teleradio-Moldova's radio channels stream from radiolive.trm.md as
/// unsigned audio-only HLS with per-channel named mounts (each verified
/// live and distinct — segments are named per mount). Actualități and
/// Comrat are served on 443, Tineret and Muzical only on :8001; the old
/// trm.md pages embed rdlive.trm.md for the main channel, a host with no
/// DNS record — radiomoldova.md is the current source of truth. Logos are
/// the 2024 rebrand marks from radiomoldova.md: plain-path SVGs in solid
/// brand colours (no rasters/filters), plus the 1024px brand PNG for the
/// main channel. Comrat is the Gagauzia regional service.
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo)
    (
        "actualitati",
        "Radio Moldova",
        "https://radiolive.trm.md/hls_rma/actualitati.m3u8",
        "https://radiomoldova.md/images/radio_logo_big.png",
    ),
    (
        "tineret",
        "Radio Moldova Tineret",
        "https://radiolive.trm.md:8001/hls_rmt/tineret.m3u8",
        "https://radiomoldova.md/images/RM_tineret_logo.svg",
    ),
    (
        "muzical",
        "Radio Moldova Muzical",
        "https://radiolive.trm.md:8001/hls_rmm/muzical.m3u8",
        "https://radiomoldova.md/images/RM_muzical_logo.svg",
    ),
    (
        "comrat",
        "Radio Moldova Comrat",
        "https://radiolive.trm.md/hls_comrat/comrat.m3u8",
        "https://radiomoldova.md/images/RM_comrat_logo.svg",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_url)| {
            debug!(provider = "trm", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "trm".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(provider = "trm", count = stations.len(), "Discovery complete");
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

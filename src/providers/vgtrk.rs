use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Russia";
const COUNTRY_CODE: &str = "RU";

/// VGTRK (the All-Russia State Television and Radio Broadcasting Company) is
/// Russia's national broadcaster. It is a **state broadcaster**, not an
/// independent public-service one — there is no equivalent of the BBC/SRF/NPO
/// in Russia — so these are included with that caveat.
///
/// Its stations stream from a single Icecast host with no channel-list
/// endpoint, so the set is pinned explicitly (like `trm`/`rtva`). The host is
/// cleartext HTTP only — TLS is not served. Reachability was verified live
/// from a UK network, and the provider is marked trusted as a
/// broadcaster-direct source.
const STREAM_BASE: &str = "http://icecast.vgtrk.cdnvideo.ru";

/// (Icecast mount and provider_id, display name, logo)
const STATIONS: &[(&str, &str, Option<&str>)] = &[
    (
        "rrzonam_mp3_192kbps",
        "Радио России",
        Some("https://upload.wikimedia.org/wikipedia/commons/b/bb/Radio_Rossii_logo.svg"),
    ),
    (
        "mayakfm_mp3_192kbps",
        "Радио Маяк",
        Some("https://upload.wikimedia.org/wikipedia/commons/5/5c/Radio_Mayak_logo.svg"),
    ),
    // No published mark found for Radio Kultura.
    ("kulturafm_mp3_192kbps", "Радио Культура", None),
    (
        "vestifm_mp3_192kbps",
        "Вести FM",
        Some("https://upload.wikimedia.org/wikipedia/commons/6/6b/Vesti_FM_logo.svg"),
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(mount, display_name, logo_url)| {
            let stream_url = format!("{STREAM_BASE}/{mount}");
            debug!(provider = "vgtrk", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: logo_url.map(str::to_string),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "vgtrk".into(),
                provider_id: Some((*mount).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "vgtrk",
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
        let mut mounts: Vec<&str> = STATIONS.iter().map(|(id, _, _)| *id).collect();
        let mut names: Vec<&str> = STATIONS.iter().map(|(_, n, _)| *n).collect();
        mounts.sort();
        mounts.dedup();
        names.sort();
        names.dedup();
        assert_eq!(mounts.len(), STATIONS.len());
        assert_eq!(names.len(), STATIONS.len());
    }
}

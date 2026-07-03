use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Belarus";
const COUNTRY_CODE: &str = "BY";

/// Belteleradio's channel artwork is unreachable (tvr.by blocks foreign
/// clients) and only Radius FM has a mark on Wikimedia Commons; the rest
/// carry the broadcaster's BTRC mark.
const BTRC_LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/d/de/BTRC.png";

/// Belteleradio's streaming host (stream2.datacenter.by, Beltelecom) serves
/// Belarus only — foreign clients get HTTP 403 for every mount, valid or
/// not, which also rules out status-page discovery from CI. The URLs below
/// are the community-proven Radio Browser entries (Radius FM alone has
/// 13,000+ votes), shipped statically: the geo-aware liveness policy exists
/// for exactly this, and trusted stations are never probed. Radio Stalitsa
/// has no discoverable stream anywhere and is the one missing channel.
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo)
    (
        "1kanal",
        "Pershy Kanal",
        "https://stream2.datacenter.by/1kanal",
        BTRC_LOGO,
    ),
    (
        "kultura",
        "Kanal Kultura",
        "https://stream2.datacenter.by/kultura",
        BTRC_LOGO,
    ),
    (
        "radiusfm",
        "Radius FM",
        "https://stream2.datacenter.by/radiusfm_main",
        "https://upload.wikimedia.org/wikipedia/commons/thumb/c/c4/Radiusfm_black.svg/960px-Radiusfm_black.svg.png",
    ),
    (
        "belarus",
        "Radio Belarus",
        "http://stream2.datacenter.by:8008/belarus",
        BTRC_LOGO,
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_url)| {
            debug!(provider = "btrc", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "btrc".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "btrc",
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

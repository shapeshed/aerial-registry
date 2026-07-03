use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Luxembourg";
const COUNTRY_CODE: &str = "LU";

/// RTL Lëtzebuerg is private (RTL Group) but holds Luxembourg's
/// public-service broadcasting mandate by state concession — the de-facto
/// national broadcaster. Streams: the main channel on RTL's own Shoutcast
/// (sc.rtl.lu — note shoutcast.rtl.lu redirects every path to the same
/// default mount and must not be used), the others on the live-edge HLS
/// load balancer that 302s to the current stream.rtl.lu edge. All verified
/// live and distinct. Gold has no Luxembourg-specific mark anywhere
/// (Commons' "RTL Gold" is the Hungarian TV channel), so it carries the
/// broadcaster mark.
const LETZEBUERG_LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/thumb/e/ec/RTL-Radio-Letzebuerg-Logo.svg/960px-RTL-Radio-Letzebuerg-Logo.svg.png";

const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, stream, logo)
    (
        "rtl",
        "RTL Radio Lëtzebuerg",
        "https://sc.rtl.lu/rtl",
        LETZEBUERG_LOGO,
    ),
    (
        "rtlgold",
        "RTL Gold",
        "https://live-edge.rtl.lu/radio/rtlgold/playlist.m3u8",
        LETZEBUERG_LOGO,
    ),
    (
        "rtltodayradio",
        "RTL Today Radio",
        "https://live-edge.rtl.lu/radio/rtltodayradio/playlist.m3u8",
        "https://upload.wikimedia.org/wikipedia/commons/9/92/Rtl-today_rgb_small.png",
    ),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, stream_url, logo_url)| {
            debug!(provider = "rtl-lu", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url: (*stream_url).to_string(),
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "rtl-lu".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "rtl-lu",
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

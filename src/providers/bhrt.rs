use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Bosnia and Herzegovina";
const COUNTRY_CODE: &str = "BA";

/// The flagship broadcast station streams from BH Telecom's CDN. The URL is
/// geo-restricted from some networks (HTTP 403) but plays fine in the region
/// — exactly the case the geo-aware liveness policy exists for; as a trusted
/// station it is never probed anyway.
const BH_RADIO_1_STREAM: &str = "https://webtvstream.bhtelecom.ba/bh_radio1.m3u8";
/// Wikimedia's PNG render of the BHRT wordmark: the raw SVG did not display
/// in the app.
const BH_RADIO_1_LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/thumb/d/d6/Logo_of_BHRT_%281998-%29.svg/960px-Logo_of_BHRT_%281998-%29.svg.png";

/// BHRT's iRadio web music channels (Art, Dance, Evergreen, Naš, Sevdah,
/// Jazz on pstnet7.shoutcastnet.com) are deliberately absent: the streaming
/// host serves an incomplete TLS chain (leaf only, no intermediate), which
/// desktop clients repair by fetching the intermediate themselves but
/// Android rejects outright — the streams cannot connect on device. Restore
/// them from git history if BHRT ever fixes the chain
/// (`openssl s_client -connect pstnet7.shoutcastnet.com:10074` should verify
/// cleanly).
pub async fn discover(_client: &Client) -> Vec<Station> {
    let station = Station {
        name: "BH Radio 1".to_string(),
        stream_url: BH_RADIO_1_STREAM.to_string(),
        logo_url: Some(BH_RADIO_1_LOGO.to_string()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: "bhrt".into(),
        provider_id: Some("bhradio1".to_string()),
        trusted: true,
    };
    debug!(provider = "bhrt", name = %station.name, stream_url = %station.stream_url, "Discovered station");

    tracing::info!(provider = "bhrt", count = 1, "Discovery complete");
    vec![station]
}

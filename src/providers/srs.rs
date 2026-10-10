use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Suriname";
const COUNTRY_CODE: &str = "SR";

/// Stichting Radio-omroep Suriname (SRS) is Suriname's state-owned radio
/// service, broadcasting on 96.3 FM since 1965. Its own site (`radiosrs.sr`) is
/// behind bot protection from the build location, but the stream — which
/// identifies itself as SRS in its ICY headers — is served by its streaming
/// host, SuriLive. There is no station-list endpoint, so the service is
/// catalogued.
const STREAM_URL: &str = "https://surilive.com:8060/;";
const LOGO: &str = "https://radio.sr/assets/srs.jpg";

pub async fn discover(_client: &Client) -> Vec<Station> {
    debug!(
        provider = "srs",
        name = "Radio SRS",
        stream = STREAM_URL,
        "Discovered station"
    );
    let stations = vec![Station {
        name: "Radio SRS".to_string(),
        stream_url: STREAM_URL.to_string(),
        logo_url: Some(LOGO.to_string()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: "srs".into(),
        provider_id: Some("srs-963".to_string()),
        trusted: true,
    }];

    tracing::info!(
        provider = "srs",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

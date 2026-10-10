use crate::http::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Peru";
const COUNTRY_CODE: &str = "PE";

/// Radio Nacional del Perú is the public radio station of the Instituto
/// Nacional de Radio y Televisión del Perú (IRTP). IRTP publishes no station
/// list — its site's JSON:API is blocked (403) and the player resolves the
/// stream client-side — so the station is catalogued.
///
/// The stream is an HLS manifest on IRTP's CDN (`iblups`) whose path is a
/// per-stream hash, served over cleartext HTTP (the host presents a TLS
/// certificate for a different name, so HTTPS fails validation). It may need
/// re-resolving if IRTP reissues the stream.
const STREAM_URL: &str = "http://cdnhd.iblups.com/hls/0773874174fd4eba8bb9eff741d190dc.m3u8";
const LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/3/36/Radio_Nacional_del_Per%C3%BA_%282021%29.svg";

pub async fn discover(_client: &Client) -> Vec<Station> {
    debug!(
        provider = "irtp",
        name = "Radio Nacional del Perú",
        stream = STREAM_URL,
        "Discovered station"
    );
    let stations = vec![Station {
        name: "Radio Nacional del Perú".to_string(),
        stream_url: STREAM_URL.to_string(),
        logo_url: Some(LOGO.to_string()),
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: "irtp".into(),
        provider_id: Some("radio-nacional".to_string()),
        trusted: true,
    }];

    tracing::info!(
        provider = "irtp",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

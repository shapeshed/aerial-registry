use crate::http::Client;
use tracing::{debug, error};

use crate::station::Station;

const COUNTRY: &str = "Ecuador";
const COUNTRY_CODE: &str = "EC";
const PROVIDER: &str = "publica-fm";

/// Pública FM is the national public radio of Ecuador (COMEP / Medios
/// Públicos). Its site is a JavaScript app whose server-rendered configuration
/// carries the stream URL as a `streamingUrl` field — a discovery source rather
/// than a catalogue.
const SITE: &str = "https://www.publicafm.ec/";
const CODE: &str = "publica-fm";
const NAME: &str = "Pública FM";

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = match client.get(SITE).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(provider = PROVIDER, "Site request failed: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            error!(provider = PROVIDER, "Site request failed: {e}");
            return Vec::new();
        }
    };

    // The config is HTML-escaped and JSON-escaped in the page source.
    let config = body.replace("&quot;", "\"").replace("\\/", "/");
    let Some(stream_url) = json_string(&config, "streamingUrl") else {
        error!(provider = PROVIDER, "No streamingUrl found in site config");
        return Vec::new();
    };

    debug!(provider = PROVIDER, name = NAME, %stream_url, "Discovered station");
    let stations = vec![Station {
        name: NAME.to_string(),
        stream_url,
        logo_url: None,
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: PROVIDER.into(),
        provider_id: Some(CODE.to_string()),
        trusted: true,
    }];

    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Return the value of the first `"key":"value"` occurrence.
fn json_string(haystack: &str, key: &str) -> Option<String> {
    let key = format!("\"{key}\"");
    let start = haystack.find(&key)? + key.len();
    let rest = &haystack[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let open = rest.find('"')?;
    let rest = &rest[open + 1..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    value.starts_with("http").then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::json_string;

    #[test]
    fn reads_streaming_url_from_escaped_config() {
        let html = "&quot;streamingUrl&quot;:&quot;https:\\/\\/comep.radioca.st\\/stream&quot;,&quot;x&quot;:1";
        let config = html.replace("&quot;", "\"").replace("\\/", "/");
        assert_eq!(
            json_string(&config, "streamingUrl").as_deref(),
            Some("https://comep.radioca.st/stream")
        );
        assert!(json_string("<html></html>", "streamingUrl").is_none());
    }
}

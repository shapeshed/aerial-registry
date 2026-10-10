use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "Paraguay";
const COUNTRY_CODE: &str = "PY";
const PROVIDER: &str = "rnp";

/// Radio Nacional del Paraguay's live-player page is a WordPress site whose
/// "Radio Player" plugin embeds each station's configuration as a base64
/// `data-data` attribute: the decoded JSON carries the station title, stream
/// URL and thumbnail. The station list is therefore discovered from the page
/// rather than catalogued, and the streams are RNP's own
/// (`audio.radionacional.gov.py`, `audio2.radionacional.gov.py`).
const LIVE_URL: &str = "http://webaudio.radionacional.gov.py/";

#[derive(Deserialize)]
struct PlayerConfig {
    #[serde(default)]
    stations: Vec<PlayerStation>,
}

#[derive(Deserialize)]
struct PlayerStation {
    title: Option<String>,
    stream: Option<String>,
    thumbnail: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let body = match client.get(LIVE_URL).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(provider = PROVIDER, "Live page request failed: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            error!(provider = PROVIDER, "Live page request failed: {e}");
            return Vec::new();
        }
    };

    let configs = player_configs(&body);
    if configs.is_empty() {
        error!(
            provider = PROVIDER,
            "No player configs found on the live page"
        );
        return Vec::new();
    }

    let mut stations = Vec::new();
    for config in configs {
        for station in config.stations {
            let Some(name) = station
                .title
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            else {
                continue;
            };
            let Some(stream_url) = station
                .stream
                .as_deref()
                .map(str::trim)
                .filter(|u| u.starts_with("http"))
            else {
                warn!(
                    provider = PROVIDER,
                    name, "Skipped station with no stream URL"
                );
                continue;
            };
            debug!(provider = PROVIDER, name, stream_url, "Discovered station");
            stations.push(Station {
                name: name.to_string(),
                stream_url: stream_url.to_string(),
                logo_url: station.thumbnail,
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: PROVIDER.into(),
                provider_id: Some(slug(name)),
                trusted: true,
            });
        }
    }

    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Extract and decode every `data-data="<base64>"` player configuration.
fn player_configs(html: &str) -> Vec<PlayerConfig> {
    let marker = "data-data=\"";
    let mut configs = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(marker) {
        let after = &rest[start + marker.len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        if let Some(bytes) = base64_decode(&after[..end])
            && let Ok(config) = serde_json::from_slice(&bytes)
        {
            configs.push(config);
        }
        rest = &after[end..];
    }
    configs
}

fn slug(name: &str) -> String {
    name.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Standard base64 (RFC 4648) with padding; stops at the first `=`.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }

    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for &c in input.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = value(c)?;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{base64_decode, player_configs, slug};

    #[test]
    fn decodes_base64() {
        assert_eq!(base64_decode("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
        assert_eq!(
            base64_decode("eyJhIjoxfQ==").as_deref(),
            Some(&b"{\"a\":1}"[..])
        );
    }

    #[test]
    fn extracts_player_stations() {
        // {"title":"920AM","stations":[{"title":"RNP 920 AM","stream":"http://audio.radionacional.gov.py/920","thumbnail":"http://x/920.jpg"}]}
        let b64 = "eyJ0aXRsZSI6IjkyMEFNIiwic3RhdGlvbnMiOlt7InRpdGxlIjoiUk5QIDkyMCBBTSIsInN0cmVhbSI6Imh0dHA6Ly9hdWRpby5yYWRpb25hY2lvbmFsLmdvdi5weS85MjAiLCJ0aHVtYm5haWwiOiJodHRwOi8veC85MjAuanBnIn1dfQ==";
        let html = format!("<div data-data=\"{b64}\" data-player-type=\"shortcode\"></div>");
        let configs = player_configs(&html);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].stations.len(), 1);
        assert_eq!(configs[0].stations[0].title.as_deref(), Some("RNP 920 AM"));
        assert_eq!(
            configs[0].stations[0].stream.as_deref(),
            Some("http://audio.radionacional.gov.py/920")
        );
        assert_eq!(slug("RNP 95.1 FM"), "rnp-95-1-fm");
    }
}

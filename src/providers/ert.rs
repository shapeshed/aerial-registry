use std::collections::HashMap;

use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// ERTecho is ERT's radio platform: every channel page's nav menu lists all
/// stations (slug + native name), and each page embeds its Icecast stream on
/// radiostreaming.ert.gr server-side. The /radio/ index responds 404 with
/// full content, so a channel page is the bootstrap.
const BOOTSTRAP_PAGE: &str = "https://www.ertecho.gr/radio/deftero/";
const CHANNEL_BASE: &str = "https://www.ertecho.gr/radio";
/// ert.gr's radios API carries square 180×180 channel logos for the
/// national and web channels (dt is mandatory; any current timestamp works).
const LOGOS_API: &str = "https://www.ert.gr/wp-json/ert/radios/playing/now";
/// The regional stations have no individual marks; ERT brands them
/// collectively as ΕΡΤ Περιφέρεια, whose logo the API carries.
const REGIONAL_LOGO_SLUG: &str = "ert-perifereia";
const COUNTRY: &str = "Greece";
const COUNTRY_CODE: &str = "GR";

#[derive(Deserialize)]
struct ApiEntry {
    radio: ApiRadio,
}

#[derive(Deserialize)]
struct ApiRadio {
    slug: String,
    logo: Option<ApiLogo>,
}

#[derive(Deserialize)]
struct ApiLogo {
    url: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let bootstrap = match fetch_page(client, BOOTSTRAP_PAGE).await {
        Some(html) => html,
        None => return vec![],
    };
    let channels = parse_channels(&bootstrap);
    if channels.is_empty() {
        error!(provider = "ert", "No channels in ERTecho nav menu");
        return vec![];
    }
    let logos = fetch_logos(client).await;
    let regional_logo = logos.get(REGIONAL_LOGO_SLUG).cloned();

    let fetches: Vec<_> = channels
        .into_iter()
        .map(|(slug, name)| {
            let client = client.clone();
            let bootstrap_stream = extract_stream(&bootstrap)
                .filter(|_| BOOTSTRAP_PAGE.contains(&format!("/{slug}/")));
            async move {
                let stream = match bootstrap_stream {
                    Some(s) => Some(s),
                    None => fetch_page(&client, &format!("{CHANNEL_BASE}/{slug}/"))
                        .await
                        .and_then(|html| extract_stream(&html)),
                };
                (slug, name, stream)
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (slug, name, stream) in join_all(fetches).await {
        let Some(stream_url) = stream else {
            warn!(provider = "ert", station = %slug, "No stream on channel page — skipping");
            continue;
        };
        let logo_url = logos.get(&slug).cloned().or_else(|| regional_logo.clone());
        debug!(provider = "ert", name = %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "ert".into(),
            provider_id: Some(slug),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "ert",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn fetch_page(client: &Client, url: &str) -> Option<String> {
    match client.get(url).send().await {
        Ok(r) => r.text().await.ok(),
        Err(e) => {
            error!(provider = "ert", url, "Failed to fetch page: {e}");
            None
        }
    }
}

/// (slug, native name) pairs from the nav menu anchors:
/// `<a href="https://www.ertecho.gr/radio/<slug>/">NAME</a>`. Card links
/// without anchor text are skipped; the first named occurrence wins.
fn parse_channels(html: &str) -> Vec<(String, String)> {
    const MARKER: &str = "href=\"https://www.ertecho.gr/radio/";
    let mut out: Vec<(String, String)> = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find(MARKER) {
        rest = &rest[pos + MARKER.len()..];
        let Some(slug_end) = rest.find('/') else {
            break;
        };
        let slug = &rest[..slug_end];
        if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let after = &rest[gt + 1..];
        let Some(lt) = after.find('<') else { break };
        let name = after[..lt].trim();
        if name.is_empty() || name.contains('{') {
            continue;
        }
        if !out.iter().any(|(s, _)| s == slug) {
            out.push((slug.to_string(), name.to_string()));
        }
    }
    out
}

fn extract_stream(html: &str) -> Option<String> {
    const PREFIX: &str = "https://radiostreaming.ert.gr/";
    let pos = html.find(PREFIX)?;
    let rest = &html[pos + PREFIX.len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(rest.len());
    (end > 0).then(|| format!("{PREFIX}{}", &rest[..end]))
}

/// Slug → logo URL from ert.gr's radios API. Best-effort: on failure the
/// stations ship without logos rather than not at all.
async fn fetch_logos(client: &Client) -> HashMap<String, String> {
    let url = format!("{LOGOS_API}?dt={}", dt_param());
    let entries: HashMap<String, ApiEntry> = match client.get(&url).send().await {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!(provider = "ert", "Failed to parse logos API: {e}");
                return HashMap::new();
            }
        },
        Err(e) => {
            warn!(provider = "ert", "Failed to fetch logos API: {e}");
            return HashMap::new();
        }
    };
    entries
        .into_values()
        .filter_map(|e| {
            let url = e.radio.logo.and_then(|l| l.url)?;
            Some((e.radio.slug, url))
        })
        .collect()
}

/// The API requires a Y-m-d-H-M timestamp. Civil-from-days per Howard
/// Hinnant's algorithm; UTC is fine, the parameter only selects a schedule
/// slot.
fn dt_param() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86400) as i64);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}-{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, extract_stream, parse_channels};

    #[test]
    fn parses_nav_channels() {
        let html = r#"
          <li><a href="https://www.ertecho.gr/radio/deftero/">ΔΕΥΤΕΡΟ ΠΡΟΓΡΑΜΜΑ</a></li>
          <li><a href="https://www.ertecho.gr/radio/chania/">ΕΡΑ ΧΑΝΙΩΝ</a></li>
          <a href="https://www.ertecho.gr/radio/deftero/"><img src="x.png"></a>
        "#;
        let channels = parse_channels(html);
        assert_eq!(
            channels,
            vec![
                ("deftero".to_string(), "ΔΕΥΤΕΡΟ ΠΡΟΓΡΑΜΜΑ".to_string()),
                ("chania".to_string(), "ΕΡΑ ΧΑΝΙΩΝ".to_string()),
            ]
        );
    }

    #[test]
    fn extracts_stream_mount() {
        let html = r#"player.load("https://radiostreaming.ert.gr/ert-talaika");"#;
        assert_eq!(
            extract_stream(html).as_deref(),
            Some("https://radiostreaming.ert.gr/ert-talaika")
        );
        assert_eq!(extract_stream("<html>none</html>"), None);
    }

    #[test]
    fn civil_conversion_is_correct() {
        // 2026-07-03 is day 20637 since the epoch.
        assert_eq!(civil_from_days(20637), (2026, 7, 3));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}

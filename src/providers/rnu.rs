use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "Uruguay";
const COUNTRY_CODE: &str = "UY";
const PROVIDER: &str = "rnu";

/// Radiodifusión Nacional del Uruguay publishes one "… en Vivo" page per
/// station on its WordPress site, each embedding an iwstreaming player widget
/// whose id maps to a stream on the same host:
///
///   https://mediospublicos.uy/wp-content/…/player/single?p=<id>  →  https://radios.iwstreaming.uy/<id>/stream
///
/// The WordPress REST API lets the station list be discovered by searching
/// page content for the widget, rather than cataloguing the four stations.
const PAGES_URL: &str = "https://mediospublicos.uy/wp-json/wp/v2/pages?search=iwstreaming&per_page=100&_fields=slug,title,content";
const WIDGET_MARKER: &str = "iwstreaming.uy/cp/widgets/player/single?p=";
const STREAM_BASE: &str = "https://radios.iwstreaming.uy";
const LOGO_BASE: &str = "https://mediospublicos.uy/wp-content/uploads";

#[derive(Deserialize)]
struct Page {
    slug: Option<String>,
    title: Option<Rendered>,
    content: Option<Rendered>,
}

#[derive(Deserialize)]
struct Rendered {
    rendered: Option<String>,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let pages = match client
        .get(PAGES_URL)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.json::<Vec<Page>>().await,
            Err(e) => {
                error!(provider = PROVIDER, "Page request failed: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            error!(provider = PROVIDER, "Page request failed: {e}");
            return Vec::new();
        }
    };

    let pages = match pages {
        Ok(pages) => pages,
        Err(e) => {
            error!(provider = PROVIDER, "Could not parse page list: {e}");
            return Vec::new();
        }
    };

    let mut stations = Vec::new();
    for page in pages {
        let content = page.content.and_then(|c| c.rendered).unwrap_or_default();
        let Some(widget_id) = widget_id(&content) else {
            warn!(provider = PROVIDER, "Skipped page with no player widget");
            continue;
        };
        let Some(name) = page
            .title
            .and_then(|t| t.rendered)
            .map(|t| clean_name(&strip_tags(&t)))
            .filter(|n| !n.is_empty())
        else {
            continue;
        };

        let stream_url = format!("{STREAM_BASE}/{widget_id}/stream");
        debug!(provider = PROVIDER, name, %stream_url, "Discovered station");
        stations.push(Station {
            name: name.clone(),
            stream_url,
            logo_url: logo_for(&name),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: PROVIDER.into(),
            provider_id: page.slug,
            trusted: true,
        });
    }

    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// `…/player/single?p=8036&t…` → `8036`.
fn widget_id(content: &str) -> Option<String> {
    let start = content.find(WIDGET_MARKER)? + WIDGET_MARKER.len();
    let digits: String = content[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!digits.is_empty()).then_some(digits)
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut depth = 0u32;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn clean_name(title: &str) -> String {
    title.trim().trim_end_matches(" en Vivo").trim().to_string()
}

fn logo_for(name: &str) -> Option<String> {
    let file = match name {
        "Radio Uruguay" => "2026/06/radio_uruguay.png",
        "Radio Clásica" => "2026/06/radio_clasica.png",
        "Radio Cultura" => "2026/06/radio_cultura.png",
        "Radio Babel" => "2021/06/radio-babel.png",
        _ => return None,
    };
    Some(format!("{LOGO_BASE}/{file}"))
}

#[cfg(test)]
mod tests {
    use super::{clean_name, logo_for, widget_id};

    #[test]
    fn extracts_widget_id_and_cleans_name() {
        let content = r#"<iframe src="https://radios.iwstreaming.uy/cp/widgets/player/single?p=8036&t=1"></iframe>"#;
        assert_eq!(widget_id(content).as_deref(), Some("8036"));
        assert_eq!(clean_name("Radio Uruguay en Vivo"), "Radio Uruguay");
        assert!(widget_id("<p>no player</p>").is_none());
        assert!(
            logo_for("Radio Babel")
                .unwrap()
                .ends_with("/2021/06/radio-babel.png")
        );
    }
}

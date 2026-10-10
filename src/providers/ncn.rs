use crate::http::Client;
use futures::future::join_all;
use tracing::{debug, error, warn};

use crate::station::Station;

const COUNTRY: &str = "Guyana";
const COUNTRY_CODE: &str = "GY";
const PROVIDER: &str = "ncn";
const BASE: &str = "https://ncnguyana.com";
const LIST_URL: &str = "https://ncnguyana.com/radio.php";

/// NCN's radio landing page lists each station with its logo, name and a
/// `listen.php?station=<CODE>` link; the linked page embeds the stream in an
/// `<audio>` `<source>`. The provider walks that two-level structure so the
/// station list is discovered rather than catalogued.
pub async fn discover(client: &Client) -> Vec<Station> {
    let body = match client.get(LIST_URL).send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                error!(provider = PROVIDER, "Listing request failed: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            error!(provider = PROVIDER, "Listing request failed: {e}");
            return Vec::new();
        }
    };

    let listings = parse_listing(&body);
    if listings.is_empty() {
        error!(provider = PROVIDER, "No stations found on the radio page");
        return Vec::new();
    }

    let fetches: Vec<_> = listings
        .iter()
        .map(|listing| {
            let client = client.clone();
            async move {
                let url = format!("{BASE}/listen.php?station={}", listing.code);
                let stream_url = match client.get(&url).send().await {
                    Ok(resp) => resp.text().await.ok().and_then(|html| source_src(&html)),
                    Err(_) => None,
                };
                (listing, stream_url)
            }
        })
        .collect();

    let mut stations = Vec::new();
    for (listing, stream_url) in join_all(fetches).await {
        let Some(stream_url) = stream_url else {
            warn!(provider = PROVIDER, station = %listing.code, "No stream found on listen page");
            continue;
        };
        debug!(provider = PROVIDER, name = %listing.name, %stream_url, "Discovered station");
        stations.push(Station {
            name: listing.name.clone(),
            stream_url,
            logo_url: listing.logo.clone(),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: PROVIDER.into(),
            provider_id: Some(listing.code.clone()),
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

struct Listing {
    code: String,
    name: String,
    logo: Option<String>,
}

fn parse_listing(html: &str) -> Vec<Listing> {
    let marker = "listen.php?station=";
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = html[search_from..].find(marker) {
        let idx = search_from + rel;
        let after = &html[idx + marker.len()..];
        let code: String = after
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        let before = &html[..idx];
        if let (false, Some(name)) = (code.is_empty(), last_div_text(before)) {
            out.push(Listing {
                code,
                name,
                logo: last_img_src(before).map(resolve_logo),
            });
        }
        search_from = idx + marker.len();
    }
    out
}

/// Text of the last closed `<div>` before `before`.
fn last_div_text(before: &str) -> Option<String> {
    let end = before.rfind("</div>")?;
    let start = before[..end].rfind("<div")?;
    let gt = before[start..end].find('>')? + start + 1;
    let text = before[gt..end].trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn last_img_src(before: &str) -> Option<String> {
    let start = before.rfind("<img")?;
    let rest = &before[start..];
    let src = rest.find("src=\"")? + "src=\"".len();
    let rest = &rest[src..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn resolve_logo(src: String) -> String {
    if src.starts_with("http") {
        src
    } else {
        format!("{BASE}/{}", src.trim_start_matches('/'))
    }
}

/// The stream URL from an `<audio><source src="…">`.
fn source_src(html: &str) -> Option<String> {
    let marker = "<source src=\"";
    let start = html.find(marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    let url = &rest[..end];
    url.starts_with("http").then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_listing, source_src};

    #[test]
    fn pairs_name_logo_and_code() {
        let html = r#"<div class="card">
            <div style="height:64px;"><img src="vog.png"><br></div>
            <div style="height:42px;">Voice of Guyana 102.5 FM</div><br>
            <div onclick="jump();window.location.href=`listen.php?station=VOG`"> <img src="play_button.svg"> </div>
        </div>"#;
        let listings = parse_listing(html);
        assert_eq!(listings.len(), 1);
        assert_eq!(listings[0].code, "VOG");
        assert_eq!(listings[0].name, "Voice of Guyana 102.5 FM");
        assert_eq!(
            listings[0].logo.as_deref(),
            Some("https://ncnguyana.com/vog.png")
        );
    }

    #[test]
    fn extracts_source_url() {
        let html = r#"<audio id="st_aud" controls><source src="https://cast4.asurahosting.com/proxy/ncn1025/stream"></audio>"#;
        assert_eq!(
            source_src(html).as_deref(),
            Some("https://cast4.asurahosting.com/proxy/ncn1025/stream")
        );
    }
}

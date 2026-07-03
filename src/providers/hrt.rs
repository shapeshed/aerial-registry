use std::collections::HashMap;

use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// HRT publishes no station API; the radio player is a Next.js app whose
/// server-rendered __NEXT_DATA__ carries the channel list with Triton
/// mount names.
const PLAYER_BASE: &str = "https://radio.hrt.hr/stream";
const COUNTRY: &str = "Croatia";
const COUNTRY_CODE: &str = "HR";

/// Streams are on Triton/StreamTheWorld; the livestream-redirect endpoint
/// 302s to a live edge and is the stable static URL form.
const STREAM_BASE: &str = "https://playerservices.streamtheworld.com/api/livestream-redirect";

/// Broadcaster mark used when a station has no artwork of its own
/// (Radio Split, Juhuhu, or a music channel whose guide card vanishes).
const FALLBACK_LOGO: &str =
    "https://upload.wikimedia.org/wikipedia/commons/7/7b/Croatian_Radio_logo.png";

/// Where a station's artwork comes from. HRT publishes no channel logos:
/// the web-only music channels brand every programme block in their guide
/// with one channel card (extracted from the station page, so it tracks HRT
/// renames), while the broadcast stations' guide images are show art shared
/// across channels — their logos come from Wikimedia Commons.
enum Logo {
    Static(&'static str),
    GuideCard,
    Fallback,
}

const STATIONS: &[(&str, &str, Logo)] = &[
    // (Triton mount, display name, logo source)
    (
        "PROGRAM1",
        "HRT Prvi program",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/a/ad/HR1_logo.jpg"),
    ),
    (
        "PROGRAM2",
        "HRT Drugi program",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/8/8e/HR2_logo.jpg"),
    ),
    (
        "PROGRAM3",
        "HRT Treći program",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/1/19/HR3_logo.jpg"),
    ),
    (
        "SLJEME",
        "HRT Radio Sljeme",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/2/22/Hrt_radio_sljeme.png"),
    ),
    (
        "OSIJEK",
        "HRT Radio Osijek",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/2/2a/Hrt_radio_osijek.png"),
    ),
    (
        "PULA",
        "HRT Radio Pula",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/1/15/Hrt_radio_pula.png"),
    ),
    (
        "RIJEKA",
        "HRT Radio Rijeka",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/d/d8/Hrt_radio_rijeka.png"),
    ),
    (
        "ZADAR",
        "HRT Radio Zadar",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/0/01/Hrt_radio_zadar.png"),
    ),
    (
        "KNIN",
        "HRT Radio Knin",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/c/c0/Hrt_radio_knin.png"),
    ),
    ("SPLIT", "HRT Radio Split", Logo::Fallback),
    (
        "DUBROVNIK",
        "HRT Radio Dubrovnik",
        Logo::Static("https://upload.wikimedia.org/wikipedia/commons/9/96/Hrt_radio_dubrovnik.png"),
    ),
    (
        "VOICEOFCROATIA",
        "HRT Glas Hrvatske",
        Logo::Static(
            "https://upload.wikimedia.org/wikipedia/commons/0/0f/HRT_Glas_Hrvatske_logo.png",
        ),
    ),
    ("HR_CLASSICS", "HRT Klasik", Logo::GuideCard),
    ("HR_POP", "HRT Pop", Logo::GuideCard),
    ("HR_KIT", "HRT Klape", Logo::GuideCard),
    ("HR_ROCK", "HR Rock", Logo::GuideCard),
    ("HRT_JUHUHU", "HRT Juhuhu", Logo::Fallback),
];

#[derive(Deserialize)]
struct NextData {
    props: Props,
}

#[derive(Deserialize)]
struct Props {
    #[serde(rename = "pageProps")]
    page_props: PageProps,
}

#[derive(Deserialize)]
struct PageProps {
    #[serde(rename = "allChannelsData", default)]
    all_channels_data: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    #[serde(rename = "MountName")]
    mount_name: String,
    #[serde(rename = "StreamId")]
    stream_id: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let html = match fetch_page(client, "6").await {
        Some(h) => h,
        None => return vec![],
    };

    let Some(channels) = parse_channels(&html) else {
        error!(
            provider = "hrt",
            "No __NEXT_DATA__ channel list in player page"
        );
        return vec![];
    };
    let stream_ids: HashMap<&str, &str> = channels
        .iter()
        .map(|c| (c.mount_name.as_str(), c.stream_id.as_str()))
        .collect();

    // Music channels brand their whole programme guide with one channel
    // card; fetch those pages concurrently and take the dominant image.
    let card_fetches: Vec<_> = STATIONS
        .iter()
        .filter(|(mount, _, logo)| {
            matches!(logo, Logo::GuideCard) && stream_ids.contains_key(mount)
        })
        .map(|(mount, _, _)| {
            let client = client.clone();
            let stream_id = stream_ids[mount].to_string();
            async move {
                let card = match fetch_page(&client, &stream_id).await {
                    Some(page) => dominant_guide_card(&page),
                    None => None,
                };
                (*mount, card)
            }
        })
        .collect();
    let cards: HashMap<&str, Option<String>> = join_all(card_fetches).await.into_iter().collect();

    let mut stations = Vec::new();
    for (mount, display_name, logo) in STATIONS {
        if !stream_ids.contains_key(mount) {
            warn!(
                provider = "hrt",
                station = mount,
                "Station missing from player"
            );
            continue;
        }
        let logo_url = match logo {
            Logo::Static(url) => Some((*url).to_string()),
            Logo::GuideCard => {
                let card = cards.get(mount).cloned().flatten();
                if card.is_none() {
                    warn!(
                        provider = "hrt",
                        station = mount,
                        "No guide card found; using broadcaster fallback"
                    );
                }
                Some(card.unwrap_or_else(|| FALLBACK_LOGO.to_string()))
            }
            Logo::Fallback => Some(FALLBACK_LOGO.to_string()),
        };
        let stream_url = format!("{STREAM_BASE}/{mount}.mp3");
        debug!(provider = "hrt", name = display_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: (*display_name).to_string(),
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "hrt".into(),
            provider_id: Some(mount.to_lowercase()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "hrt",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn fetch_page(client: &Client, stream_id: &str) -> Option<String> {
    match client
        .get(format!("{PLAYER_BASE}/{stream_id}"))
        .send()
        .await
    {
        Ok(r) => match r.text().await {
            Ok(t) => Some(t),
            Err(e) => {
                error!(
                    provider = "hrt",
                    stream_id, "Failed to read player page: {e}"
                );
                None
            }
        },
        Err(e) => {
            error!(
                provider = "hrt",
                stream_id, "Failed to fetch player page: {e}"
            );
            None
        }
    }
}

fn parse_channels(html: &str) -> Option<Vec<Channel>> {
    let marker = "id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find("</script>")? + start;
    let data: NextData = serde_json::from_str(&html[start..end]).ok()?;
    let channels = data.props.page_props.all_channels_data;
    (!channels.is_empty()).then_some(channels)
}

/// The image every (or nearly every) programme block shares is the channel
/// card. Requiring at least two occurrences rejects pages whose guide only
/// has per-show art.
fn dominant_guide_card(html: &str) -> Option<String> {
    const PREFIX: &str = "https://api.hrt.hr/media/";
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut rest = html;
    while let Some(pos) = rest.find(PREFIX) {
        let candidate = &rest[pos..];
        let end = candidate
            .find(|c: char| c == '"' || c == '\'' || c == '\\' || c == '<' || c.is_whitespace())
            .unwrap_or(candidate.len());
        let url = &candidate[..end];
        if url.ends_with(".webp") {
            *counts.entry(url).or_insert(0) += 1;
        }
        rest = &rest[pos + PREFIX.len()..];
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .max_by_key(|(_, count)| *count)
        .map(|(url, _)| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::{dominant_guide_card, parse_channels};

    #[test]
    fn parses_channels_from_next_data() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"allChannelsData":[{"DisplayName":"Prvi program","MountName":"PROGRAM1","StreamId":"6"}]}}}</script></html>"#;
        let channels = parse_channels(html).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].mount_name, "PROGRAM1");
        assert_eq!(channels[0].stream_id, "6");
    }

    #[test]
    fn missing_next_data_is_none() {
        assert!(parse_channels("<html><body>nope</body></html>").is_none());
    }

    #[test]
    fn dominant_card_needs_repetition() {
        let page = r#"
            "img":"https://api.hrt.hr/media/aa/67/pop-1.webp"
            "img":"https://api.hrt.hr/media/aa/67/pop-1.webp"
            "img":"https://api.hrt.hr/media/bb/00/show-2.webp"
        "#;
        assert_eq!(
            dominant_guide_card(page).as_deref(),
            Some("https://api.hrt.hr/media/aa/67/pop-1.webp")
        );
        // Only one-off show images: no channel card.
        let per_show =
            r#""https://api.hrt.hr/media/aa/1/a.webp" "https://api.hrt.hr/media/bb/2/b.webp""#;
        assert_eq!(dominant_guide_card(per_show), None);
    }
}

use futures::future::join_all;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::station::Station;

const BASE_URL: &str = "https://www.abc.net.au/listen/live";
const COUNTRY: &str = "Australia";
const COUNTRY_CODE: &str = "AU";

/// ABC has no public API for station discovery or stream resolution — each
/// live page is a Next.js SPA that embeds the player config (stream URL,
/// logo) in a `__NEXT_DATA__` JSON blob. There's also no endpoint that lists
/// all stations; this covers every national network plus the state-capital
/// ABC Local stations. It deliberately excludes ABC's ~50 further
/// regional/rural Local stations, which have no cleaner discovery path than
/// guessing city-name slugs one at a time.
const STATIONS: &[(&str, &str)] = &[
    ("triplej", "Triple J"),
    ("doublej", "Double J"),
    ("unearthed", "Triple J Unearthed"),
    ("classic", "ABC Classic"),
    ("country", "ABC Country"),
    ("jazz", "ABC Jazz"),
    ("kidslisten", "ABC Kids Listen"),
    ("radionational", "ABC Radio National"),
    ("news", "ABC NewsRadio"),
    ("sport", "ABC Sport"),
    ("radioaustralia", "ABC Radio Australia"),
    ("sydney", "ABC Radio Sydney"),
    ("melbourne", "ABC Radio Melbourne"),
    ("brisbane", "ABC Radio Brisbane"),
    ("adelaide", "ABC Radio Adelaide"),
    ("perth", "ABC Radio Perth"),
    ("hobart", "ABC Radio Hobart"),
    ("canberra", "ABC Radio Canberra"),
    ("darwin", "ABC Radio Darwin"),
    ("newcastle", "ABC Radio Newcastle"),
];

/// Recursively searches a parsed `__NEXT_DATA__` tree for the player config
/// object, identified by having both `papiServiceId` and `config.sources` —
/// its position in the tree varies by page, so a fixed path isn't reliable.
fn find_player_config(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            let has_sources = map.get("config").and_then(|c| c.get("sources")).is_some();
            if map.contains_key("papiServiceId") && has_sources {
                return Some(value);
            }
            map.values().find_map(find_player_config)
        }
        Value::Array(items) => items.iter().find_map(find_player_config),
        _ => None,
    }
}

fn extract_next_data(html: &str) -> Option<Value> {
    let tag = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html.find(tag)? + tag.len();
    let end = html[start..].find("</script>")? + start;
    serde_json::from_str(&html[start..end]).ok()
}

async fn fetch_station(client: &Client, slug: &str, name: &str) -> Option<Station> {
    let url = format!("{BASE_URL}/{slug}");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        warn!(provider = "abc", slug, "Live page not reachable — skipping");
        return None;
    }
    let html = resp.text().await.ok()?;
    let data = extract_next_data(&html)?;
    let config = find_player_config(&data)?;

    let stream_url = config
        .get("config")?
        .get("sources")?
        .as_array()?
        .first()?
        .get("file")?
        .as_str()?
        .to_string();
    let provider_id = config
        .get("papiServiceId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let logo_url = config
        .get("radioHeadingPrepared")
        .and_then(|v| v.get("logoPrepared"))
        .and_then(|v| v.get("imgSrc"))
        .and_then(Value::as_str)
        .map(str::to_string);

    debug!(provider = "abc", %name, %stream_url, "Discovered station");
    Some(Station {
        name: name.to_string(),
        stream_url,
        logo_url,
        country: Some(COUNTRY.into()),
        country_code: Some(COUNTRY_CODE.into()),
        tags: vec![],
        description: None,
        provider: "abc".into(),
        provider_id,
        trusted: true,
    })
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches = STATIONS
        .iter()
        .map(|&(slug, name)| fetch_station(client, slug, name));
    let stations: Vec<Station> = join_all(fetches).await.into_iter().flatten().collect();

    if stations.len() < STATIONS.len() {
        error!(
            provider = "abc",
            found = stations.len(),
            expected = STATIONS.len(),
            "Some ABC stations failed to resolve"
        );
    }

    tracing::info!(
        provider = "abc",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

use std::collections::HashMap;
use std::env;

use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::station::Station;

const GRAPHQL_URL: &str = "https://openapi.radiofrance.fr/v1/graphql";
const BRAND_PAGE_BASE: &str = "https://www.radiofrance.fr";
const API_KEY_ENV: &str = "RADIO_FRANCE_API_KEY";
const QUERY: &str = "{ brands { id title baseline description liveStream \
    webRadios { id title description liveStream } \
    localRadios { id title description liveStream } } }";

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
}

#[derive(Deserialize)]
struct GqlData {
    brands: Vec<Brand>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Brand {
    id: String,
    title: String,
    baseline: Option<String>,
    description: Option<String>,
    live_stream: Option<String>,
    web_radios: Option<Vec<SubRadio>>,
    local_radios: Option<Vec<SubRadio>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubRadio {
    id: String,
    title: String,
    description: Option<String>,
    live_stream: Option<String>,
}

/// Extracts the SVG brand logo from the SvelteKit asset URL embedded in the brand's page HTML.
/// The URL contains a content-hash fingerprint (e.g. `franceinter.DWjOK1w1.svg`) that changes
/// on redeploy, so it must be extracted fresh each run.
async fn fetch_brand_logo(client: &Client, brand_id: &str) -> Option<String> {
    let slug = brand_id.to_lowercase();
    let page_url = format!("{BRAND_PAGE_BASE}/{slug}");
    let html = client.get(&page_url).send().await.ok()?.text().await.ok()?;
    let prefix = format!("/_app/immutable/assets/{slug}.");
    let start = html.find(&prefix)?;
    let after_prefix = start + prefix.len();
    let svg_end = html[after_prefix..].find(".svg")? + after_prefix + 4;
    let path = &html[start..svg_end];
    Some(format!("{BRAND_PAGE_BASE}{path}"))
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let api_key = match env::var(API_KEY_ENV) {
        Ok(k) if !k.is_empty() => k,
        _ => {
            error!(provider = "radio-france", "{API_KEY_ENV} not set — skipping");
            return vec![];
        }
    };

    let body = serde_json::json!({ "query": QUERY });

    let resp = match client
        .post(GRAPHQL_URL)
        .header("x-token", &api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "radio-france", "Failed to fetch brands: {e}");
            return vec![];
        }
    };

    let gql: GqlResponse = match resp.json().await {
        Ok(g) => g,
        Err(e) => {
            error!(provider = "radio-france", "Failed to parse response: {e}");
            return vec![];
        }
    };

    let brands = match gql.data {
        Some(d) => d.brands,
        None => {
            error!(provider = "radio-france", "No data in response");
            return vec![];
        }
    };

    // Fetch all brand logos concurrently — one og:image request per brand page.
    let logo_fetches: Vec<_> = brands
        .iter()
        .map(|brand| {
            let client = client.clone();
            let brand_id = brand.id.clone();
            async move {
                let logo = fetch_brand_logo(&client, &brand_id).await;
                if logo.is_none() {
                    warn!(provider = "radio-france", brand = %brand_id, "Could not fetch brand logo");
                }
                (brand_id, logo)
            }
        })
        .collect();

    let logo_map: HashMap<String, String> = join_all(logo_fetches)
        .await
        .into_iter()
        .filter_map(|(id, logo)| logo.map(|l| (id, l)))
        .collect();

    debug!(provider = "radio-france", logos = logo_map.len(), "Fetched brand logos");

    let mut stations = Vec::new();

    for brand in brands {
        let logo_url = logo_map.get(&brand.id).cloned();

        // Main brand stream
        if let Some(stream_url) = brand.live_stream.filter(|u| !u.is_empty()) {
            let description = brand
                .baseline
                .filter(|s| !s.is_empty())
                .or_else(|| brand.description.clone().filter(|s| !s.is_empty()));
            debug!(provider = "radio-france", name = %brand.title, %stream_url, "Discovered station");
            stations.push(Station {
                name: brand.title.clone(),
                stream_url,
                logo_url: logo_url.clone(),
                country: Some("France".into()),
                country_code: Some("FR".into()),
                tags: vec![],
                description,
                provider: "radio-france".into(),
                provider_id: Some(brand.id.clone()),
                trusted: true,
            });
        }

        // Thematic web radio sub-channels (e.g. FIP Rock, FIP Jazz, France Musique Classique Easy)
        for radio in brand.web_radios.unwrap_or_default() {
            let Some(stream_url) = radio.live_stream.filter(|u| !u.is_empty()) else {
                continue;
            };
            let name = qualify_name(&brand.title, &radio.title);
            let description = radio.description.filter(|s| !s.is_empty());
            debug!(provider = "radio-france", %name, %stream_url, "Discovered web radio");
            stations.push(Station {
                name,
                stream_url,
                logo_url: logo_url.clone(),
                country: Some("France".into()),
                country_code: Some("FR".into()),
                tags: vec![],
                description,
                provider: "radio-france".into(),
                provider_id: Some(radio.id),
                trusted: true,
            });
        }

        // Regional stations (ICI / France Bleu network)
        for radio in brand.local_radios.unwrap_or_default() {
            let Some(stream_url) = radio.live_stream.filter(|u| !u.is_empty()) else {
                continue;
            };
            let name = qualify_name(&brand.title, &radio.title);
            let description = radio.description.filter(|s| !s.is_empty());
            debug!(provider = "radio-france", %name, %stream_url, "Discovered local radio");
            stations.push(Station {
                name,
                stream_url,
                logo_url: logo_url.clone(),
                country: Some("France".into()),
                country_code: Some("FR".into()),
                tags: vec![],
                description,
                provider: "radio-france".into(),
                provider_id: Some(radio.id),
                trusted: true,
            });
        }
    }

    info!(provider = "radio-france", count = stations.len(), "Discovery complete");
    stations
}

/// Prefix sub-radio title with the brand title if it doesn't already begin with it.
/// "FIP Rock" stays "FIP Rock"; "Classique Easy" becomes "France Musique - Classique Easy".
fn qualify_name(brand: &str, sub: &str) -> String {
    if sub.to_lowercase().starts_with(&brand.to_lowercase()) {
        sub.to_owned()
    } else {
        format!("{brand} - {sub}")
    }
}

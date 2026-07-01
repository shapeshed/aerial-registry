use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const STATIONS_URL: &str = "https://bff-web-guacamole.musicradio.com/stations/";
const HOMEPAGE_URL: &str = "https://www.globalplayer.com/";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalStation {
    id: Option<String>,
    slug: Option<String>,
    gduid: Option<String>,
    name: Option<String>,
    stream_url: Option<String>,
    stream: Option<GlobalStream>,
    tagline: Option<String>,
    brand: Option<GlobalBrand>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalStream {
    icecast_sd: Option<String>,
}

#[derive(Deserialize)]
struct GlobalBrand {
    slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextDataPage {
    page_props: Option<NextDataProps>,
}

#[derive(Deserialize)]
struct NextDataProps {
    station: Option<NextDataStation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextDataStation {
    brand_logo: Option<String>,
}

async fn fetch_build_id(client: &Client) -> Option<String> {
    let html = client
        .get(HOMEPAGE_URL)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let key = "\"buildId\":\"";
    let start = html.find(key)? + key.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_owned())
}

async fn fetch_brand_logo(
    client: &Client,
    build_id: &str,
    brand_slug: &str,
    station_slug: &str,
) -> Option<String> {
    let url = format!(
        "https://www.globalplayer.com/_next/data/{build_id}/live/{brand_slug}/{station_slug}.json\
         ?brand={brand_slug}&station={station_slug}"
    );
    let page: NextDataPage = client.get(&url).send().await.ok()?.json().await.ok()?;
    page.page_props?
        .station?
        .brand_logo
        .filter(|u| !u.is_empty())
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let build_id = match fetch_build_id(client).await {
        Some(id) => id,
        None => {
            warn!(
                provider = "global",
                "Could not fetch build ID — logos will be absent"
            );
            String::new()
        }
    };

    let resp = match client.get(STATIONS_URL).send().await {
        Ok(r) => r,
        Err(e) => {
            error!(provider = "global", "Failed to fetch stations: {e}");
            return vec![];
        }
    };

    let raw: Vec<GlobalStation> = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!(provider = "global", "Failed to parse response: {e}");
            return vec![];
        }
    };

    // One (brand_slug, station_slug) pair per brand — all stations in a brand share the logo.
    let mut brand_rep: HashMap<String, String> = HashMap::new();
    for s in &raw {
        if let (Some(brand), Some(slug)) = (s.brand.as_ref(), s.slug.as_deref()) {
            brand_rep
                .entry(brand.slug.clone())
                .or_insert_with(|| slug.to_owned());
        }
    }

    let logo_map: HashMap<String, String> = if build_id.is_empty() {
        HashMap::new()
    } else {
        let futures: Vec<_> = brand_rep
            .iter()
            .map(|(brand_slug, station_slug)| {
                let client = client.clone();
                let build_id = build_id.clone();
                let brand_slug = brand_slug.clone();
                let station_slug = station_slug.clone();
                async move {
                    let logo =
                        fetch_brand_logo(&client, &build_id, &brand_slug, &station_slug).await;
                    (brand_slug, logo)
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        results
            .into_iter()
            .filter_map(|(slug, logo)| logo.map(|l| (slug, l)))
            .collect()
    };

    debug!(
        provider = "global",
        brands = logo_map.len(),
        "Fetched brand logos"
    );

    let mut stations = Vec::new();

    for s in raw {
        let name = match s.name.filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => continue,
        };
        let stream_url = match s.stream_url.filter(|u| !u.is_empty()).or_else(|| {
            s.stream
                .and_then(|st| st.icecast_sd)
                .filter(|u| !u.is_empty())
        }) {
            Some(u) => u,
            None => continue,
        };

        let logo_url = s
            .brand
            .as_ref()
            .and_then(|b| logo_map.get(&b.slug))
            .cloned();

        debug!(provider = "global", %name, %stream_url, "Discovered station");
        stations.push(Station {
            name,
            stream_url,
            logo_url,
            country: Some("United Kingdom".into()),
            country_code: Some("GB".into()),
            tags: vec![],
            description: s.tagline.filter(|t| !t.is_empty()),
            provider: "global".into(),
            provider_id: s
                .gduid
                .filter(|v| !v.is_empty())
                .or_else(|| s.id.filter(|v| !v.is_empty())),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "global",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

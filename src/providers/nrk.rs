use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

const METADATA_URL: &str = "https://psapi.nrk.no/playback/metadata/channel";
const MANIFEST_URL: &str = "https://psapi.nrk.no/playback/manifest/channel";

/// NRK's public playback API has no endpoint that enumerates all radio
/// channels — this list was assembled by probing known channel ID patterns
/// (national/thematic channels, plus every P1 district edition).
const CHANNEL_IDS: &[&str] = &[
    "p1",
    "p2",
    "p3",
    "mp3",
    "klassisk",
    "jazz",
    "radio_super",
    "alltid_nyheter",
    "folkemusikk",
    "sport",
    "p1pluss",
    "sapmi",
    "p1_buskerud",
    "p1_finnmark",
    "p1_hordaland",
    "p1_innlandet",
    "p1_more_romsdal",
    "p1_nordland",
    "p1_oslo_akershus",
    "p1_ostfold",
    "p1_rogaland",
    "p1_sogn_fjordane",
    "p1_sorlandet",
    "p1_telemark",
    "p1_troms",
    "p1_trondelag",
    "p1_vestfold",
];

#[derive(Deserialize)]
struct MetadataResponse {
    preplay: Preplay,
}

#[derive(Deserialize)]
struct Preplay {
    titles: Titles,
    description: Option<String>,
    poster: Option<Poster>,
}

#[derive(Deserialize)]
struct Titles {
    title: String,
}

#[derive(Deserialize)]
struct Poster {
    images: Vec<PosterImage>,
}

#[derive(Deserialize)]
struct PosterImage {
    url: String,
    #[serde(rename = "pixelWidth")]
    pixel_width: i64,
}

#[derive(Deserialize)]
struct ManifestResponse {
    playable: Option<Playable>,
}

#[derive(Deserialize)]
struct Playable {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    url: String,
}

/// NRK serves each poster at several fixed widths (no arbitrary resizing) —
/// pick whichever is closest to a reasonable thumbnail size.
fn pick_logo(images: Vec<PosterImage>) -> Option<String> {
    images
        .into_iter()
        .min_by_key(|i| (i.pixel_width - 600).abs())
        .map(|i| i.url)
}

async fn fetch_channel(client: &Client, id: &str) -> Option<Station> {
    let metadata_url = format!("{METADATA_URL}/{id}");
    let manifest_url = format!("{MANIFEST_URL}/{id}");

    let (metadata_resp, manifest_resp) = tokio::join!(
        client.get(&metadata_url).send(),
        client.get(&manifest_url).send()
    );

    let metadata: MetadataResponse = match metadata_resp {
        Ok(r) => r.json().await.ok()?,
        Err(e) => {
            warn!(
                provider = "nrk",
                channel_id = id,
                "Failed to fetch metadata: {e}"
            );
            return None;
        }
    };
    let manifest: ManifestResponse = match manifest_resp {
        Ok(r) => r.json().await.ok()?,
        Err(e) => {
            warn!(
                provider = "nrk",
                channel_id = id,
                "Failed to fetch manifest: {e}"
            );
            return None;
        }
    };

    let Some(stream_url) = manifest
        .playable
        .and_then(|p| p.assets.into_iter().next())
        .map(|a| a.url)
    else {
        warn!(
            provider = "nrk",
            channel_id = id,
            "No stream asset in manifest — skipping"
        );
        return None;
    };

    let name = metadata.preplay.titles.title;
    let description = metadata.preplay.description.filter(|d| !d.is_empty());
    let logo_url = metadata.preplay.poster.and_then(|p| pick_logo(p.images));

    debug!(provider = "nrk", %name, %stream_url, "Discovered station");
    Some(Station {
        name,
        stream_url,
        logo_url,
        country: Some("Norway".into()),
        country_code: Some("NO".into()),
        tags: vec![],
        description,
        provider: "nrk".into(),
        provider_id: Some(id.to_string()),
        trusted: true,
    })
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let fetches = CHANNEL_IDS.iter().map(|&id| fetch_channel(client, id));
    let stations: Vec<Station> = join_all(fetches).await.into_iter().flatten().collect();

    if stations.len() < CHANNEL_IDS.len() {
        error!(
            provider = "nrk",
            found = stations.len(),
            expected = CHANNEL_IDS.len(),
            "Some NRK channel IDs failed to resolve"
        );
    }

    tracing::info!(
        provider = "nrk",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

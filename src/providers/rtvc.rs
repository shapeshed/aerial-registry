use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error};

use crate::station::Station;

const COUNTRY: &str = "Colombia";
const COUNTRY_CODE: &str = "CO";
const PROVIDER: &str = "rtvc";

/// RTVC's "Parrilla" Drupal site is the back end for its public-media channel
/// lineup, and it exposes a JSON:API. Each channel node carries an HLS URL
/// (`field_streaming_hls`) and a logo file; RTVC names its radio streams
/// `Radio_*`, which is what distinguishes them from the television channels in
/// the same list.
const CHANNELS_URL: &str =
    "https://parrilla.rtvc.gov.co/jsonapi/node/channel?page%5Blimit%5D=50&include=field_logo_json";

#[derive(Deserialize)]
struct Document {
    #[serde(default)]
    data: Vec<Resource>,
    #[serde(default)]
    included: Vec<Resource>,
}

#[derive(Deserialize)]
struct Resource {
    id: String,
    #[serde(default)]
    attributes: Attributes,
    #[serde(default)]
    relationships: HashMap<String, Relationship>,
}

#[derive(Deserialize, Default)]
struct Attributes {
    title: Option<String>,
    field_streaming_hls: Option<String>,
    #[serde(rename = "drupal_internal__nid")]
    nid: Option<i64>,
    uri: Option<Uri>,
}

#[derive(Deserialize)]
struct Uri {
    url: Option<String>,
}

#[derive(Deserialize)]
struct Relationship {
    data: Option<RelationshipData>,
}

#[derive(Deserialize)]
struct RelationshipData {
    id: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let document = match client
        .get(CHANNELS_URL)
        .header("Accept", "application/vnd.api+json")
        .send()
        .await
    {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => resp.json::<Document>().await,
            Err(e) => {
                error!(provider = PROVIDER, "Channel request failed: {e}");
                return Vec::new();
            }
        },
        Err(e) => {
            error!(provider = PROVIDER, "Channel request failed: {e}");
            return Vec::new();
        }
    };

    let document = match document {
        Ok(document) => document,
        Err(e) => {
            error!(provider = PROVIDER, "Could not parse channel document: {e}");
            return Vec::new();
        }
    };

    let stations = radio_stations(&document);
    tracing::info!(
        provider = PROVIDER,
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

fn radio_stations(document: &Document) -> Vec<Station> {
    // File resources are referenced by id; build the id → URL map once.
    let files: HashMap<&str, &str> = document
        .included
        .iter()
        .filter_map(|r| {
            let url = r.attributes.uri.as_ref()?.url.as_deref()?;
            Some((r.id.as_str(), url))
        })
        .collect();

    let mut stations = Vec::new();
    for channel in &document.data {
        let attrs = &channel.attributes;
        let Some(stream_url) = attrs.field_streaming_hls.as_deref() else {
            continue;
        };
        // RTVC's radio streams live under "Radio_"; the TV channels and the
        // radio-studio webcams do not.
        if !stream_url.contains("/Radio_") {
            continue;
        }
        let Some(name) = attrs
            .title
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };

        let logo_url = channel
            .relationships
            .get("field_logo_json")
            .and_then(|r| r.data.as_ref())
            .and_then(|d| files.get(d.id.as_str()))
            .map(|u| (*u).to_string());

        debug!(provider = PROVIDER, name, stream_url, "Discovered station");
        stations.push(Station {
            name: name.to_string(),
            stream_url: stream_url.to_string(),
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: PROVIDER.into(),
            provider_id: attrs.nid.map(|n| n.to_string()),
            trusted: true,
        });
    }
    stations
}

#[cfg(test)]
mod tests {
    use super::{Document, radio_stations};

    #[test]
    fn keeps_only_radio_channels_and_resolves_logos() {
        let json = r#"{
          "data": [
            { "id": "n1", "attributes": { "title": "Radio Nacional de Colombia",
                "field_streaming_hls": "https://streaming.rtvc.gov.co/Radio_Radionacional/Radionacional.stream/playlist.m3u8",
                "drupal_internal__nid": 1848 },
              "relationships": { "field_logo_json": { "data": { "type": "file--file", "id": "f1" } } } },
            { "id": "n2", "attributes": { "title": "Señal Colombia",
                "field_streaming_hls": "https://streaming.rtvc.gov.co/TV_Senal_Colombia_live/smil:live.smil/playlist.m3u8" },
              "relationships": {} }
          ],
          "included": [ { "id": "f1", "attributes": { "uri": { "url": "https://s3.amazonaws.com/radional.png" } } } ]
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        let stations = radio_stations(&doc);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].name, "Radio Nacional de Colombia");
        assert_eq!(
            stations[0].logo_url.as_deref(),
            Some("https://s3.amazonaws.com/radional.png")
        );
        assert_eq!(stations[0].provider_id.as_deref(), Some("1848"));
        assert!(stations[0].trusted);
    }
}

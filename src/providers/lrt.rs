use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// LRT's live REST API lists every live channel (TV and radio) with a
/// per-channel stream resolver URL carrying the current HLS URL. TV
/// resolvers also return an `audio` field (the TV sound track), so radio
/// channels are selected by the explicit table below — which doubles as the
/// logo map, all LRT's own brand assets.
const LIVE_API: &str = "https://www.lrt.lt/rest-api/live/lrt-radijas";
const COUNTRY: &str = "Lithuania";
const COUNTRY_CODE: &str = "LT";

/// channelName → LRT logo asset; also the radio whitelist.
const RADIO_CHANNELS: &[(&str, &str)] = &[
    ("LR", "https://www.lrt.lt/images/logo/logo-radijas.svg"),
    ("Klasika", "https://www.lrt.lt/images/logo/logo-klasika.svg"),
    ("Opus", "https://www.lrt.lt/images/logo/logo-opus.svg"),
    ("LRT100", "https://www.lrt.lt/images/logo/logo-lrt-100.svg"),
];

#[derive(Deserialize)]
struct Live {
    #[serde(rename = "liveChannels", default)]
    live_channels: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    #[serde(rename = "channelName")]
    channel_name: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    #[serde(rename = "streamUrl")]
    stream_url: String,
}

#[derive(Deserialize)]
struct Resolver {
    response: ResolverResponse,
}

#[derive(Deserialize)]
struct ResolverResponse {
    data: ResolverData,
}

#[derive(Deserialize)]
struct ResolverData {
    #[serde(default)]
    audio: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let live: Live = match client.get(LIVE_API).send().await {
        Ok(r) => match r.json().await {
            Ok(l) => l,
            Err(e) => {
                error!(provider = "lrt", "Failed to parse live API: {e}");
                return vec![];
            }
        },
        Err(e) => {
            error!(provider = "lrt", "Failed to fetch live API: {e}");
            return vec![];
        }
    };

    let radio_channels: Vec<Channel> = live
        .live_channels
        .into_iter()
        .filter(|c| {
            RADIO_CHANNELS
                .iter()
                .any(|(name, _)| *name == c.channel_name)
        })
        .collect();
    let resolutions = join_all(radio_channels.into_iter().map(|channel| {
        let client = client.clone();
        async move {
            let audio = resolve_audio(&client, &channel.stream_url).await;
            (channel, audio)
        }
    }))
    .await;

    let mut stations = Vec::new();
    for (channel, audio) in resolutions {
        let Some(stream_url) = audio else {
            warn!(provider = "lrt", channel = %channel.channel_name, "No audio stream — skipping");
            continue;
        };
        let logo_url = RADIO_CHANNELS
            .iter()
            .find(|(name, _)| *name == channel.channel_name)
            .map(|(_, url)| (*url).to_string());
        debug!(provider = "lrt", name = %channel.channel_title, %stream_url, "Discovered station");
        stations.push(Station {
            name: channel.channel_title,
            stream_url,
            logo_url,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "lrt".into(),
            provider_id: Some(channel.channel_name.to_lowercase()),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "lrt",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

async fn resolve_audio(client: &Client, resolver_url: &str) -> Option<String> {
    let resolver: Resolver = match client.get(resolver_url).send().await {
        Ok(r) => r.json().await.ok()?,
        Err(e) => {
            error!(
                provider = "lrt",
                resolver_url, "Failed to fetch resolver: {e}"
            );
            return None;
        }
    };
    let audio = resolver.response.data.audio;
    (!audio.is_empty() && audio.starts_with("https://")).then_some(audio)
}

#[cfg(test)]
mod tests {
    use super::{Live, Resolver};

    #[test]
    fn live_api_deserializes() {
        let json = r#"{"dailyQuestion":null,"liveChannels":[{"channelName":"LR","channelTitle":"LRT Radijas","streamUrl":"https://www.lrt.lt/servisai/stream_url/live/get_live_url.php?channel=LR","other":1}]}"#;
        let live: Live = serde_json::from_str(json).unwrap();
        assert_eq!(live.live_channels.len(), 1);
        assert_eq!(live.live_channels[0].channel_name, "LR");
    }

    #[test]
    fn resolver_audio_field_deserializes() {
        let json = r#"{"response":{"data":{"content":"https://x/master.m3u8","audio":"https://stream-live.lrt.lt/radio_radijas/lrt_radijas.m3u8","restriction":""}}}"#;
        let r: Resolver = serde_json::from_str(json).unwrap();
        assert_eq!(
            r.response.data.audio,
            "https://stream-live.lrt.lt/radio_radijas/lrt_radijas.m3u8"
        );
        // TV entries carry no audio field.
        let tv: Resolver =
            serde_json::from_str(r#"{"response":{"data":{"content":"https://x/tv.m3u8"}}}"#)
                .unwrap();
        assert!(tv.response.data.audio.is_empty());
    }
}

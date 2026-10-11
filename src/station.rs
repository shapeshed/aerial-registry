use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub stream_url: String,
    /// Additional fixed-bitrate mounts of the same station (ICY/Icecast), so the
    /// app can pick one to match available bandwidth. Empty for single-stream
    /// and HLS (adaptive) stations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub streams: Vec<StreamVariant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        default,
        deserialize_with = "deserialize_tags"
    )]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Not written to the registry. Trusted provider stations skip liveness checks.
    #[serde(skip)]
    pub trusted: bool,
}

/// A single playable stream for a station. `bitrate` is kbps where known.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamVariant {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Best-effort bitrate (kbps) parsed from a stream URL, e.g. `…-128-mp3` or
/// `…_320.aac`. Only accepts plausible audio values (16–512, multiple of 8),
/// and returns the last match so a bitrate suffix wins over other numbers.
pub fn bitrate_from_url(url: &str) -> Option<u32> {
    let lower = url.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut best = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(value) = lower[start..i].parse::<u32>()
                && (16..=512).contains(&value)
                && value % 8 == 0
            {
                best = Some(value);
            }
        } else {
            i += 1;
        }
    }
    best
}

/// Own output always writes `tags` as a JSON array, but a `Station` list
/// loaded from elsewhere (`pipeline::guard`'s "previous registry" file can
/// be any registry.json a human points it at) may use a different shape —
/// the app's own bundled registry stores tags as one space-separated
/// string. Accept either.
fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TagsRepr {
        List(Vec<String>),
        Joined(String),
    }
    Ok(match TagsRepr::deserialize(deserializer)? {
        TagsRepr::List(tags) => tags,
        TagsRepr::Joined(s) => s.split_whitespace().map(str::to_string).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_accept_a_json_array() {
        let s: Station = serde_json::from_str(
            r#"{"name":"X","stream_url":"https://x","tags":["pop","rock"],"provider":"curated"}"#,
        )
        .unwrap();
        assert_eq!(s.tags, vec!["pop".to_string(), "rock".to_string()]);
    }

    #[test]
    fn tags_accept_a_space_separated_string() {
        let s: Station = serde_json::from_str(
            r#"{"name":"X","stream_url":"https://x","tags":"dance electronic house","provider":"radio-browser"}"#,
        )
        .unwrap();
        assert_eq!(
            s.tags,
            vec![
                "dance".to_string(),
                "electronic".to_string(),
                "house".to_string()
            ]
        );
    }

    #[test]
    fn tags_default_to_empty_when_absent() {
        let s: Station =
            serde_json::from_str(r#"{"name":"X","stream_url":"https://x","provider":"curated"}"#)
                .unwrap();
        assert!(s.tags.is_empty());
    }
}

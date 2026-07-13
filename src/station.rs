use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub stream_url: String,
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

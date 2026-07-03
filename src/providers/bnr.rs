use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, warn};

use crate::station::Station;

/// BNR pages embed a `LongPrograms` JSON model carrying every programme's
/// native name, slug and live stream — the streams are stable load-balancer
/// URLs on lb-hls.cdn.bg that 302 to the current edge. binar.bg (BNR's
/// online platform) serves the model without the anti-bot friction of the
/// main site.
const SOURCE_PAGE: &str = "https://binar.bg/";
const COUNTRY: &str = "Bulgaria";
const COUNTRY_CODE: &str = "BG";

/// Broadcaster mark for programmes without a channel logo on Commons
/// (Хоризонт, Христо Ботев, Трафик+); Wikimedia PNG render of the BNR SVG.
const BNR_LOGO: &str = "https://upload.wikimedia.org/wikipedia/commons/thumb/4/4a/Bulgarian_National_Radio.svg/960px-Bulgarian_National_Radio.svg.png";

/// Commons has marks for all nine regional stations, keyed by BNR's own
/// UrlTitle slug.
const LOGOS: &[(&str, &str)] = &[
    (
        "sofia",
        "https://upload.wikimedia.org/wikipedia/commons/c/c5/BNR_Sofia_logo.png",
    ),
    (
        "blagoevgrad",
        "https://upload.wikimedia.org/wikipedia/commons/2/21/BNR_Blagoevgrad_logo.png",
    ),
    (
        "burgas",
        "https://upload.wikimedia.org/wikipedia/commons/4/42/BNR_Burgas_logo.png",
    ),
    (
        "varna",
        "https://upload.wikimedia.org/wikipedia/commons/f/f3/BNR_Varna_logo.png",
    ),
    (
        "vidin",
        "https://upload.wikimedia.org/wikipedia/commons/a/ab/BNR_Vidin_logo.png",
    ),
    (
        "kardzhali",
        "https://upload.wikimedia.org/wikipedia/commons/f/f6/BNR_Kardzhali_logo.png",
    ),
    (
        "plovdiv",
        "https://upload.wikimedia.org/wikipedia/commons/e/e4/BNR_Plovdiv_logo.png",
    ),
    (
        "shumen",
        "https://upload.wikimedia.org/wikipedia/commons/6/6e/BNR_Shumen_logo.png",
    ),
    (
        "starazagora",
        "https://upload.wikimedia.org/wikipedia/commons/e/e8/BNR_Stara_Zagora_logo.png",
    ),
];

#[derive(Deserialize)]
struct Program {
    #[serde(rename = "ProgramShortName")]
    short_name: String,
    #[serde(rename = "ProgramStream")]
    stream: Option<String>,
    #[serde(rename = "UrlTitle")]
    url_title: String,
}

pub async fn discover(client: &Client) -> Vec<Station> {
    let html = match client.get(SOURCE_PAGE).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => {
            error!(provider = "bnr", "Failed to fetch source page: {e}");
            return vec![];
        }
    };
    let Some(programs) = parse_programs(&html) else {
        error!(provider = "bnr", "No LongPrograms model in source page");
        return vec![];
    };

    let mut stations = Vec::new();
    for program in programs {
        let Some(stream_url) = program.stream.filter(|s| !s.is_empty()) else {
            continue; // non-streaming programmes (e.g. the news desk)
        };
        if program.short_name.trim().is_empty() {
            warn!(provider = "bnr", slug = %program.url_title, "Programme has no name — skipping");
            continue;
        }
        let logo = LOGOS
            .iter()
            .find(|(slug, _)| *slug == program.url_title)
            .map(|(_, url)| *url)
            .unwrap_or(BNR_LOGO);
        debug!(provider = "bnr", name = %program.short_name, %stream_url, "Discovered station");
        stations.push(Station {
            name: program.short_name,
            stream_url,
            logo_url: Some(logo.to_string()),
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: vec![],
            description: None,
            provider: "bnr".into(),
            provider_id: Some(program.url_title),
            trusted: true,
        });
    }

    tracing::info!(
        provider = "bnr",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

/// Extract the `"LongPrograms": [...]` array by bracket matching — it sits
/// inside a larger inline state blob, not a standalone script tag.
fn parse_programs(html: &str) -> Option<Vec<Program>> {
    let key = html.find("\"LongPrograms\":")?;
    let start = html[key..].find('[')? + key;
    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in html[start..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let programs: Vec<Program> = serde_json::from_str(&html[start..end?]).ok()?;
    (!programs.is_empty()).then_some(programs)
}

#[cfg(test)]
mod tests {
    use super::parse_programs;

    #[test]
    fn parses_long_programs_blob() {
        let html = r#"{"other":1,"LongPrograms":[{"Id":1,"ProgramShortName":"Хоризонт","ProgramStream":"https://lb-hls.cdn.bg/2032/fls/Horizont.stream/playlist.m3u8","UrlTitle":"horizont"},{"Id":23,"ProgramShortName":"Новините","ProgramStream":null,"UrlTitle":"main"}],"tail":2}"#;
        let programs = parse_programs(html).unwrap();
        assert_eq!(programs.len(), 2);
        assert_eq!(programs[0].short_name, "Хоризонт");
        assert!(programs[1].stream.is_none());
    }

    #[test]
    fn missing_model_is_none() {
        assert!(parse_programs("<html>nothing</html>").is_none());
    }
}

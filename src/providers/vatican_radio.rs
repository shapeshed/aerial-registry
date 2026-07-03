use reqwest::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Vatican City";
const COUNTRY_CODE: &str = "VA";

/// Radio Vaticana's language services all stream from the broadcaster's own
/// Icecast at radio.vaticannews.va/stream-<lang> — the same URL the Vatican
/// News web player constructs per language edition. Unknown codes 404 (no
/// catch-all default mount), and each of these 30 codes was verified live;
/// editions without a code here (Japanese, Korean, Hebrew, Esperanto, …) are
/// online-only and have no radio stream. Names carry the language endonym;
/// the Italian flagship keeps its official "Radio Vaticana Italia" brand.
const LOGO: &str = "https://www.vaticannews.va/etc/designs/vatican-news/release/library/main/images/favicons/android-icon-192x192.png";

const SERVICES: &[(&str, &str)] = &[
    // (language code = provider_id and stream mount, display name)
    ("it", "Radio Vaticana Italia"),
    ("am", "Radio Vaticana አማርኛ"),
    ("ar", "Radio Vaticana العربية"),
    ("be", "Radio Vaticana Беларуская"),
    ("bg", "Radio Vaticana Български"),
    ("cs", "Radio Vaticana Čeština"),
    ("de", "Radio Vaticana Deutsch"),
    ("en", "Radio Vaticana English"),
    ("es", "Radio Vaticana Español"),
    ("fr", "Radio Vaticana Français"),
    ("hi", "Radio Vaticana हिन्दी"),
    ("hr", "Radio Vaticana Hrvatski"),
    ("hu", "Radio Vaticana Magyar"),
    ("hy", "Radio Vaticana Հայերեն"),
    ("lt", "Radio Vaticana Lietuvių"),
    ("lv", "Radio Vaticana Latviešu"),
    ("ml", "Radio Vaticana മലയാളം"),
    ("pl", "Radio Vaticana Polski"),
    ("pt", "Radio Vaticana Português"),
    ("ro", "Radio Vaticana Română"),
    ("ru", "Radio Vaticana Русский"),
    ("sk", "Radio Vaticana Slovenčina"),
    ("sl", "Radio Vaticana Slovenščina"),
    ("sq", "Radio Vaticana Shqip"),
    ("sw", "Radio Vaticana Kiswahili"),
    ("ta", "Radio Vaticana தமிழ்"),
    ("ti", "Radio Vaticana ትግርኛ"),
    ("uk", "Radio Vaticana Українська"),
    ("vi", "Radio Vaticana Tiếng Việt"),
    ("zh", "Radio Vaticana 中文"),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = SERVICES
        .iter()
        .map(|(code, display_name)| {
            let stream_url = format!("https://radio.vaticannews.va/stream-{code}");
            debug!(provider = "vatican-radio", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: Some(LOGO.to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "vatican-radio".into(),
                provider_id: Some((*code).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "vatican-radio",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::SERVICES;

    #[test]
    fn station_identities_are_distinct() {
        let mut codes: Vec<&str> = SERVICES.iter().map(|(c, _)| *c).collect();
        let mut names: Vec<&str> = SERVICES.iter().map(|(_, n)| *n).collect();
        codes.sort();
        codes.dedup();
        names.sort();
        names.dedup();
        assert_eq!(codes.len(), SERVICES.len());
        assert_eq!(names.len(), SERVICES.len());
    }
}

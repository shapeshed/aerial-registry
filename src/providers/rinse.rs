use reqwest::Client;

use crate::station::Station;

const COUNTRY: &str = "United Kingdom";
const COUNTRY_CODE: &str = "GB";

struct RinseStation {
    name: &'static str,
    stream_url: &'static str,
    tags: &'static [&'static str],
}

/// Rinse's live channel lineup is embedded as Craft CMS entry data inside a
/// React Server Components payload on rinse.fm's homepage — there is no
/// public API to query it directly, and no stable channel-list endpoint was
/// found (guessed REST/GraphQL paths under rinse.fm/api and rinse.fm/graphql
/// all 404). This list was extracted from that embedded payload and verified
/// directly against each stream.
///
/// Rinse France runs on different infrastructure (radio10.pro-fhi.net) from
/// the other three, which share admin.stream.rinse.fm.
const STATIONS: &[RinseStation] = &[
    RinseStation {
        name: "Rinse FM",
        stream_url: "https://admin.stream.rinse.fm/proxy/rinse_uk/stream",
        tags: &["electronic", "grime", "bass", "dance"],
    },
    RinseStation {
        name: "Rinse France",
        stream_url: "https://radio10.pro-fhi.net/flux-trmqtiat/stream",
        tags: &["electronic", "techno", "bass"],
    },
    RinseStation {
        name: "Kool FM",
        stream_url: "https://admin.stream.rinse.fm/proxy/kool/stream",
        tags: &["drum and bass", "jungle"],
    },
    RinseStation {
        name: "SWU FM",
        stream_url: "https://admin.stream.rinse.fm/proxy/swu/stream",
        tags: &["reggae", "dub", "hip-hop"],
    },
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|s| Station {
            name: s.name.to_string(),
            stream_url: s.stream_url.to_string(),
            logo_url: None,
            country: Some(COUNTRY.into()),
            country_code: Some(COUNTRY_CODE.into()),
            tags: s.tags.iter().map(|t| t.to_string()).collect(),
            description: None,
            provider: "rinse".into(),
            provider_id: None,
            trusted: true,
        })
        .collect();

    tracing::info!(
        provider = "rinse",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

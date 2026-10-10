use crate::http::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Ukraine";
const COUNTRY_CODE: &str = "UA";

/// Suspilne (UA:PBC) streams its radio channels from a single Icecast host,
/// `radio.ukr.radio`, one mount per service. The site publishes no channel-list
/// JSON: the player reads each channel's mounts from its own page
/// (`ukr.radio/channel.html?channelID=N`), and the Icecast `status.xsl` lists
/// ~90 low-level mounts including every regional opt-out of Ukrainian Radio.
/// This provider catalogues the eight national services those pages expose,
/// using each channel's MP3 mount.
const STREAM_BASE: &str = "https://radio.ukr.radio";
const LOGO_SUSPILNE: &str = "https://upload.wikimedia.org/wikipedia/commons/1/11/Suspilne_logo.svg";

/// (Icecast mount and provider_id, display name, logo)
const STATIONS: &[(&str, &str, &str)] = &[
    (
        "ur1-mp3",
        "Українське Радіо",
        "https://upload.wikimedia.org/wikipedia/commons/c/c9/Radio_Ukraine_2022.svg",
    ),
    (
        "ur2-mp3",
        "Радіо Промінь",
        "https://upload.wikimedia.org/wikipedia/commons/6/6c/Suspilne_Radio_Promin_%282022%29.svg",
    ),
    (
        "ur3-mp3",
        "Радіо Культура",
        "https://upload.wikimedia.org/wikipedia/commons/5/52/Radio_Kultura_%282022%29.svg",
    ),
    (
        "ur4-mp3",
        "Radio Ukraine International",
        "https://upload.wikimedia.org/wikipedia/commons/5/5c/Radio_Ukraine_International_2022.svg",
    ),
    (
        "ur5-mp3",
        "Радіоточка",
        "https://upload.wikimedia.org/wikipedia/commons/3/34/Suspilne_Radiotochka_2022.svg",
    ),
    // The two Kultura spin-offs and Radio Tysa have no separable mark of their
    // own (Suspilne bundles its channel art client-side and it is not
    // individually addressable), so they carry the shared Suspilne mark.
    ("urkazka-mp3", "Радіо Культура Казка", LOGO_SUSPILNE),
    ("urclassic-mp3", "Радіо Культура Класика", LOGO_SUSPILNE),
    ("tysafm-mp3", "Радіо Тиса", LOGO_SUSPILNE),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(mount, display_name, logo_url)| {
            let stream_url = format!("{STREAM_BASE}/{mount}");
            debug!(provider = "suspilne", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "suspilne".into(),
                provider_id: Some((*mount).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "suspilne",
        count = stations.len(),
        "Discovery complete"
    );
    stations
}

#[cfg(test)]
mod tests {
    use super::STATIONS;

    #[test]
    fn station_identities_are_distinct() {
        let mut mounts: Vec<&str> = STATIONS.iter().map(|(id, _, _)| *id).collect();
        let mut names: Vec<&str> = STATIONS.iter().map(|(_, n, _)| *n).collect();
        mounts.sort();
        mounts.dedup();
        names.sort();
        names.dedup();
        assert_eq!(mounts.len(), STATIONS.len());
        assert_eq!(names.len(), STATIONS.len());
    }
}

use crate::http::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Slovakia";
const COUNTRY_CODE: &str = "SK";

/// STVR streams its radio services from its own Icecast host, one mount per
/// service and bitrate. The host's `status-json.xsl` is malformed (Icecast
/// emits an unquoted `"title":-` for a source with no now-playing title, which
/// breaks JSON parsing) and `status.xsl` only lists the low-level
/// `<Service>_<bitrate>.mp3` mounts with no service names, so the set is
/// pinned explicitly (like `trm`/`rtva`). Mounts are HTTP-only: the host
/// refuses TLS on :8000 and serves nothing on :443.
const STREAM_BASE: &str = "http://live.slovakradio.sk:8000";
const LOGO_BASE: &str = "https://www.stvr.sk/media/images/radiostations";
const LOGO_STVR: &str = "https://www.stvr.sk/media/images/STVR.png";

/// (Icecast mount and provider_id, display name, logo)
const STATIONS: &[(&str, &str, &str)] = &[
    ("Slovensko_128.mp3", "Rádio Slovensko", "slovensko_2026.svg"),
    (
        "Regina_BA_128.mp3",
        "Rádio Regina Bratislava",
        "regina_zapad_2026.svg",
    ),
    (
        "Regina_BB_128.mp3",
        "Rádio Regina Banská Bystrica",
        "regina_stred_2026.svg",
    ),
    (
        "Regina_KE_128.mp3",
        "Rádio Regina Košice",
        "regina_vychod_2026.svg",
    ),
    ("Devin_128.mp3", "Rádio Devín", "devin_2026.svg"),
    ("FM_128.mp3", "Radio_FM", "fm_2026.svg"),
    // No published mark for Rádio Klasika; it carries the STVR logo.
    ("Klasika_128.mp3", "Rádio Klasika", ""),
    ("Litera_128.mp3", "Rádio Litera", "litera_2026.svg"),
    ("Patria_128.mp3", "Rádio Patria", "patria_2026.svg"),
    (
        "RSI_128.mp3",
        "Radio Slovakia International",
        "rsi_2026.svg",
    ),
    ("Junior_128.mp3", "Rádio Junior", "junior_2026.svg"),
];

fn logo_url(file: &str) -> String {
    if file.is_empty() {
        LOGO_STVR.to_string()
    } else {
        format!("{LOGO_BASE}/{file}")
    }
}

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(mount, display_name, logo)| {
            let stream_url = format!("{STREAM_BASE}/{mount}");
            debug!(provider = "stvr", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: Some(logo_url(logo)),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "stvr".into(),
                provider_id: Some((*mount).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "stvr",
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

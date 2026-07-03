use reqwest::Client;
use tracing::debug;

use crate::station::Station;

/// Yle serves its live radio channels from its own Icecast (AAC), the same
/// URLs the community has relied on for years; the Areena HLS URLs carry
/// per-channel Akamai event ids with no public resolver. Logos come from
/// Yle's image CDN using the channel image ids the Areena radio guide
/// publishes (version-less URLs serve the current artwork).
const STREAM_BASE: &str = "https://icecast.live.yle.fi/radio";
const LOGO_BASE: &str = "https://images.cdn.yle.fi/image/upload/w_512";
const COUNTRY: &str = "Finland";
const COUNTRY_CODE: &str = "FI";

/// Yle Mondo is absent: its Icecast mount is gone and no working stream
/// exists on the Akamai hosts either. Radio Suomi and Vega ship as their
/// flagship (Helsinki) feeds; the ~20 regional variants are a possible
/// later addition.
const STATIONS: &[(&str, &str, &str, &str)] = &[
    // (provider_id, display name, icecast mount, CDN image id)
    ("radio1", "Yle Radio 1", "YleRadio1", "yle-radio-1_vt"),
    ("ylex", "YleX", "YleX", "ylex_vt"),
    (
        "radio-suomi",
        "Yle Radio Suomi",
        "YleRS",
        "yle-radio-suomi-helsinki_vtc",
    ),
    (
        "klassinen",
        "Yle Klassinen",
        "YleKlassinen",
        "yle-klassinen_vt",
    ),
    ("sami", "Yle Sámi Radio", "YleSami", "yle-sami-radio_vt"),
    (
        "vega",
        "Yle Vega",
        "YleVega",
        "radio-vega-huvudstadsregionen_vtc",
    ),
    ("x3m", "Yle X3M", "YleX3M", "yle-x3m_vt"),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(id, display_name, mount, image_id)| {
            let stream_url = format!("{STREAM_BASE}/{mount}/icecast.audio");
            debug!(provider = "yle", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: Some(format!("{LOGO_BASE}/{image_id}.png")),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "yle".into(),
                provider_id: Some((*id).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "yle",
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
        let mut ids: Vec<&str> = STATIONS.iter().map(|(id, _, _, _)| *id).collect();
        let mut mounts: Vec<&str> = STATIONS.iter().map(|(_, _, m, _)| *m).collect();
        ids.sort();
        ids.dedup();
        mounts.sort();
        mounts.dedup();
        assert_eq!(ids.len(), STATIONS.len());
        assert_eq!(mounts.len(), STATIONS.len());
    }
}

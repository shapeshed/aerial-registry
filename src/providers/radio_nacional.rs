use crate::http::Client;
use tracing::debug;

use crate::station::Station;

const COUNTRY: &str = "Argentina";
const COUNTRY_CODE: &str = "AR";

/// RTA (Radio y Televisión Argentina) streams the Radio Nacional network from a
/// single Icecast host, one named mount per station (`sc_radN`). There is no
/// public station-list endpoint: the broadcaster's WordPress REST API and the
/// Icecast admin both require authentication, and its own player at
/// `/reproductor/` is not reachable from outside. The mount set below was
/// enumerated by probing the host and cross-checked against the broadcaster's
/// published emisora list (`LRA`/`LT`/`LU`/`LV` station numbers) and each
/// mount's `icy-name`/`icy-description` headers.
///
/// The five national services (Buenos Aires AM 870, Clásica, Folklórica, Rock
/// and RAE) carry their own artwork from RTA's player assets; every regional
/// station falls back to the shared Radio Nacional mark.
const STREAM_BASE: &str = "https://sa.mp3.icecast.magma.edge-access.net";

const LOGO_NACIONAL: &str =
    "https://cdn.radionacional.com.ar/wp-content/uploads/2025/06/LogoRadioNacional3.png";
const LOGO_BUENOS_AIRES: &str =
    "https://cdn.radionacional.com.ar/reproductor/images/Logo_AM870_b.png";
const LOGO_CLASICA: &str = "https://cdn.radionacional.com.ar/reproductor/images/Logo_Clasica.png";
const LOGO_FOLKLORICA: &str =
    "https://cdn.radionacional.com.ar/reproductor/images/Logo_Folklorica.png";
const LOGO_ROCK: &str = "https://cdn.radionacional.com.ar/reproductor/images/Logo_Rock.png";
const LOGO_RAE: &str = "https://cdn.radionacional.com.ar/reproductor/images/Logo_RAE.png";

/// (Icecast mount and provider_id, display name, logo)
const STATIONS: &[(&str, &str, &str)] = &[
    ("sc_rad1", "Radio Nacional Buenos Aires", LOGO_BUENOS_AIRES),
    ("sc_rad37", "Nacional Clásica", LOGO_CLASICA),
    ("sc_rad38", "Nacional Folklórica", LOGO_FOLKLORICA),
    ("sc_rad39", "Nacional Rock", LOGO_ROCK),
    ("sc_rad35", "RAE Argentina al Mundo", LOGO_RAE),
    ("sc_rad2", "Radio Nacional Viedma", LOGO_NACIONAL),
    ("sc_rad3", "Radio Nacional Santa Rosa", LOGO_NACIONAL),
    ("sc_rad4", "Radio Nacional Salta", LOGO_NACIONAL),
    ("sc_rad5", "Radio Nacional Rosario", LOGO_NACIONAL),
    ("sc_rad6", "Radio Nacional Mendoza", LOGO_NACIONAL),
    ("sc_rad7", "Radio Nacional Córdoba", LOGO_NACIONAL),
    ("sc_rad8", "Radio Nacional Formosa", LOGO_NACIONAL),
    ("sc_rad9", "Radio Nacional Esquel", LOGO_NACIONAL),
    ("sc_rad10", "Radio Nacional Ushuaia", LOGO_NACIONAL),
    (
        "sc_rad11",
        "Radio Nacional Comodoro Rivadavia",
        LOGO_NACIONAL,
    ),
    ("sc_rad12", "Radio Nacional Santo Tomé", LOGO_NACIONAL),
    ("sc_rad13", "Radio Nacional Bahía Blanca", LOGO_NACIONAL),
    ("sc_rad14", "Radio Nacional Santa Fe", LOGO_NACIONAL),
    ("sc_rad15", "Radio Nacional Tucumán", LOGO_NACIONAL),
    ("sc_rad17", "Radio Nacional Zapala", LOGO_NACIONAL),
    ("sc_rad18", "Radio Nacional Río Turbio", LOGO_NACIONAL),
    (
        "sc_rad21",
        "Radio Nacional Santiago del Estero",
        LOGO_NACIONAL,
    ),
    ("sc_rad22", "Radio Nacional Jujuy", LOGO_NACIONAL),
    ("sc_rad23", "Radio Nacional San Juan", LOGO_NACIONAL),
    ("sc_rad24", "Radio Nacional Río Grande", LOGO_NACIONAL),
    ("sc_rad26", "Radio Nacional Resistencia", LOGO_NACIONAL),
    ("sc_rad27", "Radio Nacional Catamarca", LOGO_NACIONAL),
    ("sc_rad28", "Radio Nacional La Rioja", LOGO_NACIONAL),
    ("sc_rad29", "Radio Nacional San Luis", LOGO_NACIONAL),
    ("sc_rad30", "Radio Nacional Bariloche", LOGO_NACIONAL),
    ("sc_rad31", "Radio Nacional Gualeguaychú", LOGO_NACIONAL),
    ("sc_rad34", "Radio Nacional Perito Moreno", LOGO_NACIONAL),
    ("sc_rad41", "Radio Nacional Rosario FM", LOGO_NACIONAL),
    (
        "sc_rad42",
        "Radio Nacional Patagonia Argentina",
        LOGO_NACIONAL,
    ),
    (
        "sc_rad43",
        "Radio Nacional Paso de los Libres",
        LOGO_NACIONAL,
    ),
    ("sc_rad44", "Radio Nacional Paraná", LOGO_NACIONAL),
    (
        "sc_rad45",
        "Radio Nacional Concepción del Uruguay",
        LOGO_NACIONAL,
    ),
    ("sc_rad46", "Radio Nacional San Rafael", LOGO_NACIONAL),
    ("sc_rad47", "Radio Nacional Malargüe", LOGO_NACIONAL),
    ("sc_rad48", "Radio Nacional Libertador", LOGO_NACIONAL),
    ("sc_rad49", "Radio Nacional El Calafate", LOGO_NACIONAL),
    ("sc_rad50", "Radio Nacional Paraná FM", LOGO_NACIONAL),
    (
        "sc_rad53",
        "Radio Nacional San Martín de los Andes",
        LOGO_NACIONAL,
    ),
    (
        "sc_rad54",
        "Radio Nacional Ingeniero Jacobacci",
        LOGO_NACIONAL,
    ),
    ("sc_rad55", "Radio Nacional Alto Río Senguer", LOGO_NACIONAL),
    ("sc_rad57", "Radio Nacional El Bolsón", LOGO_NACIONAL),
    ("sc_rad58", "Radio Nacional Río Mayo", LOGO_NACIONAL),
    (
        "sc_rad59",
        "Radio Nacional Gobernador Gregores",
        LOGO_NACIONAL,
    ),
    ("sc_rad70", "Radio Nacional Córdoba FM", LOGO_NACIONAL),
    ("sc_rad73", "Radio Nacional Neuquén", LOGO_NACIONAL),
    ("sc_rad102", "Radio Nacional Jáchal", LOGO_NACIONAL),
    ("sc_rad103", "Radio Nacional Chos Malal", LOGO_NACIONAL),
];

pub async fn discover(_client: &Client) -> Vec<Station> {
    let stations: Vec<Station> = STATIONS
        .iter()
        .map(|(mount, display_name, logo_url)| {
            let stream_url = format!("{STREAM_BASE}/{mount}");
            debug!(provider = "radio-nacional", name = display_name, %stream_url, "Discovered station");
            Station {
                name: (*display_name).to_string(),
                stream_url,
                logo_url: Some((*logo_url).to_string()),
                country: Some(COUNTRY.into()),
                country_code: Some(COUNTRY_CODE.into()),
                tags: vec![],
                description: None,
                provider: "radio-nacional".into(),
                provider_id: Some((*mount).to_string()),
                trusted: true,
            }
        })
        .collect();

    tracing::info!(
        provider = "radio-nacional",
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

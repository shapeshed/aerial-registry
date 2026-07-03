pub mod abc;
pub mod ard;
pub mod bauer;
pub mod bbc;
pub mod bhrt;
pub mod cbc;
pub mod cesky_rozhlas;
pub mod curated;
pub mod dr;
pub mod global;
pub mod hrt;
pub mod npo;
pub mod nrk;
pub mod orf;
pub mod polskie_radio;
pub mod radio_france;
pub mod rai;
pub mod rinse;
pub mod rtbf;
pub mod rte;
pub mod rtp;
pub mod rtsh;
pub mod rtva;
pub mod rtve;
pub mod rtvslo;
pub mod ruv;
pub mod sbs;
pub mod sr;
pub mod wireless;
pub mod yle;

use crate::station::Station;
use tracing::info;

/// Run discovery across every provider concurrently.
pub async fn discover_all(client: &reqwest::Client) -> Vec<Station> {
    info!("Starting provider discovery");

    let (
        abc,
        ard,
        bbc,
        bauer,
        bhrt,
        cbc,
        cesky_rozhlas,
        curated,
        dr,
        global,
        hrt,
        npo,
        nrk,
        orf,
        polskie_radio,
        radio_france,
        rai,
        rinse,
        rtbf,
        rte,
        rtp,
        rtsh,
        rtva,
        rtve,
        rtvslo,
        ruv,
        sbs,
        sr,
        wireless,
        yle,
    ) = tokio::join!(
        abc::discover(client),
        ard::discover(client),
        bbc::discover(client),
        bauer::discover(client),
        bhrt::discover(client),
        cbc::discover(client),
        cesky_rozhlas::discover(client),
        curated::discover(client),
        dr::discover(client),
        global::discover(client),
        hrt::discover(client),
        npo::discover(client),
        nrk::discover(client),
        orf::discover(client),
        polskie_radio::discover(client),
        radio_france::discover(client),
        rai::discover(client),
        rinse::discover(client),
        rtbf::discover(client),
        rte::discover(client),
        rtp::discover(client),
        rtsh::discover(client),
        rtva::discover(client),
        rtve::discover(client),
        rtvslo::discover(client),
        ruv::discover(client),
        sbs::discover(client),
        sr::discover(client),
        wireless::discover(client),
        yle::discover(client),
    );

    let all: Vec<_> = [
        abc,
        ard,
        bbc,
        bauer,
        bhrt,
        cbc,
        cesky_rozhlas,
        curated,
        dr,
        global,
        hrt,
        npo,
        nrk,
        orf,
        polskie_radio,
        radio_france,
        rai,
        rinse,
        rtbf,
        rte,
        rtp,
        rtsh,
        rtva,
        rtve,
        rtvslo,
        ruv,
        sbs,
        sr,
        wireless,
        yle,
    ]
    .into_iter()
    .flatten()
    .collect();
    info!(total = all.len(), "All providers complete");
    all
}

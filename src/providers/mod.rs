pub mod abc;
pub mod ard;
pub mod bauer;
pub mod bbc;
pub mod cbc;
pub mod curated;
pub mod dr;
pub mod global;
pub mod npo;
pub mod nrk;
pub mod orf;
pub mod radio_france;
pub mod rai;
pub mod rinse;
pub mod rtbf;
pub mod rte;
pub mod rtp;
pub mod rtve;
pub mod sbs;
pub mod sr;
pub mod wireless;

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
        cbc,
        curated,
        dr,
        global,
        npo,
        nrk,
        orf,
        radio_france,
        rai,
        rinse,
        rtbf,
        rte,
        rtp,
        rtve,
        sbs,
        sr,
        wireless,
    ) = tokio::join!(
        abc::discover(client),
        ard::discover(client),
        bbc::discover(client),
        bauer::discover(client),
        cbc::discover(client),
        curated::discover(client),
        dr::discover(client),
        global::discover(client),
        npo::discover(client),
        nrk::discover(client),
        orf::discover(client),
        radio_france::discover(client),
        rai::discover(client),
        rinse::discover(client),
        rtbf::discover(client),
        rte::discover(client),
        rtp::discover(client),
        rtve::discover(client),
        sbs::discover(client),
        sr::discover(client),
        wireless::discover(client),
    );

    let all: Vec<_> = [
        abc,
        ard,
        bbc,
        bauer,
        cbc,
        curated,
        dr,
        global,
        npo,
        nrk,
        orf,
        radio_france,
        rai,
        rinse,
        rtbf,
        rte,
        rtp,
        rtve,
        sbs,
        sr,
        wireless,
    ]
    .into_iter()
    .flatten()
    .collect();
    info!(total = all.len(), "All providers complete");
    all
}

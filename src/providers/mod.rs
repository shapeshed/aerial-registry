pub mod abc;
pub mod ard;
pub mod bauer;
pub mod bbc;
pub mod bhrt;
pub mod bnr;
pub mod btrc;
pub mod cbc;
pub mod cesky_rozhlas;
pub mod curated;
pub mod dr;
pub mod ebc;
pub mod err;
pub mod ert;
pub mod fluxfm;
pub mod global;
pub mod hrt;
pub mod irtp;
pub mod latvijas_radio;
pub mod lrt;
pub mod mrt;
pub mod mtva;
pub mod ncn;
pub mod npo;
pub mod nrk;
pub mod orf;
pub mod polskie_radio;
pub mod publica_fm;
pub mod radio_browser;
pub mod radio_france;
pub mod radio_nacional;
pub mod radio_paradise;
pub mod radio_romania;
pub mod rai;
pub mod rik;
pub mod rinse;
pub mod rnu;
pub mod rtbf;
pub mod rte;
pub mod rtk;
pub mod rtl_lu;
pub mod rtp;
pub mod rts;
pub mod rtsh;
pub mod rtva;
pub mod rtvc;
pub mod rtve;
pub mod rtvslo;
pub mod ruv;
pub mod sbs;
pub mod somafm;
pub mod sr;
pub mod srgssr;
pub mod srs;
pub mod stvr;
pub mod suspilne;
pub mod trm;
pub mod vatican_radio;
pub mod vgtrk;
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
        bnr,
        btrc,
        cbc,
        cesky_rozhlas,
        curated,
        dr,
        ebc,
        err,
        ert,
        fluxfm,
        global,
        hrt,
        irtp,
        latvijas_radio,
        lrt,
        mrt,
        mtva,
        ncn,
        npo,
        nrk,
        orf,
        polskie_radio,
        publica_fm,
        radio_browser,
        radio_france,
        radio_nacional,
        radio_paradise,
        radio_romania,
        rai,
        rik,
        rinse,
        rnu,
        rtbf,
        rtk,
        rtl_lu,
        rte,
        rtp,
        rts,
        rtsh,
        rtva,
        rtvc,
        rtve,
        rtvslo,
        ruv,
        sbs,
        somafm,
        sr,
        srgssr,
        srs,
        stvr,
        suspilne,
        trm,
        vatican_radio,
        vgtrk,
        wireless,
        yle,
    ) = tokio::join!(
        abc::discover(client),
        ard::discover(client),
        bbc::discover(client),
        bauer::discover(client),
        bhrt::discover(client),
        bnr::discover(client),
        btrc::discover(client),
        cbc::discover(client),
        cesky_rozhlas::discover(client),
        curated::discover(client),
        dr::discover(client),
        ebc::discover(client),
        err::discover(client),
        ert::discover(client),
        fluxfm::discover(client),
        global::discover(client),
        hrt::discover(client),
        irtp::discover(client),
        latvijas_radio::discover(client),
        lrt::discover(client),
        mrt::discover(client),
        mtva::discover(client),
        ncn::discover(client),
        npo::discover(client),
        nrk::discover(client),
        orf::discover(client),
        polskie_radio::discover(client),
        publica_fm::discover(client),
        radio_browser::discover(client),
        radio_france::discover(client),
        radio_nacional::discover(client),
        radio_paradise::discover(client),
        radio_romania::discover(client),
        rai::discover(client),
        rik::discover(client),
        rinse::discover(client),
        rnu::discover(client),
        rtbf::discover(client),
        rtk::discover(client),
        rtl_lu::discover(client),
        rte::discover(client),
        rtp::discover(client),
        rts::discover(client),
        rtsh::discover(client),
        rtva::discover(client),
        rtvc::discover(client),
        rtve::discover(client),
        rtvslo::discover(client),
        ruv::discover(client),
        sbs::discover(client),
        somafm::discover(client),
        sr::discover(client),
        srgssr::discover(client),
        srs::discover(client),
        stvr::discover(client),
        suspilne::discover(client),
        trm::discover(client),
        vatican_radio::discover(client),
        vgtrk::discover(client),
        wireless::discover(client),
        yle::discover(client),
    );

    let all: Vec<_> = [
        abc,
        ard,
        bbc,
        bauer,
        bhrt,
        bnr,
        btrc,
        cbc,
        cesky_rozhlas,
        curated,
        dr,
        ebc,
        err,
        ert,
        fluxfm,
        global,
        hrt,
        irtp,
        latvijas_radio,
        lrt,
        mrt,
        mtva,
        ncn,
        npo,
        nrk,
        orf,
        polskie_radio,
        publica_fm,
        radio_browser,
        radio_france,
        radio_nacional,
        radio_paradise,
        radio_romania,
        rai,
        rik,
        rinse,
        rnu,
        rtbf,
        rtk,
        rtl_lu,
        rte,
        rtp,
        rts,
        rtsh,
        rtva,
        rtvc,
        rtve,
        rtvslo,
        ruv,
        sbs,
        somafm,
        sr,
        srgssr,
        srs,
        stvr,
        suspilne,
        trm,
        vatican_radio,
        vgtrk,
        wireless,
        yle,
    ]
    .into_iter()
    .flatten()
    .collect();
    info!(total = all.len(), "All providers complete");
    all
}

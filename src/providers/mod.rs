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
pub mod icecast_yp;
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
pub mod rnp;
pub mod rnu;
pub mod rnv;
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

use std::collections::HashSet;

use futures::future::BoxFuture;
use tracing::info;

use crate::station::Station;

/// The subset of providers to run, from `AERIAL_PROVIDERS`: a comma-separated
/// list of provider slugs matching the `provider` field written to the registry
/// (e.g. `global`, `bbc`, `radio-france`). Unset or empty runs every provider,
/// which is the normal nightly behaviour.
fn provider_filter() -> Option<HashSet<String>> {
    let value = std::env::var("AERIAL_PROVIDERS").ok()?;
    let wanted: HashSet<String> = value
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    (!wanted.is_empty()).then_some(wanted)
}

/// Run discovery across every provider concurrently, or the subset named in
/// `AERIAL_PROVIDERS`.
pub async fn discover_all(client: &crate::http::Client) -> Vec<Station> {
    let filter = provider_filter();
    let selected = |name: &str| filter.as_ref().is_none_or(|w| w.contains(name));

    match &filter {
        Some(wanted) => info!(providers = ?wanted, "Starting provider discovery (filtered)"),
        None => info!("Starting provider discovery"),
    }

    let mut futures: Vec<BoxFuture<'_, Vec<Station>>> = Vec::new();
    if selected("abc") {
        futures.push(Box::pin(abc::discover(client)));
    }
    if selected("ard") {
        futures.push(Box::pin(ard::discover(client)));
    }
    if selected("bauer") {
        futures.push(Box::pin(bauer::discover(client)));
    }
    if selected("bbc") {
        futures.push(Box::pin(bbc::discover(client)));
    }
    if selected("bhrt") {
        futures.push(Box::pin(bhrt::discover(client)));
    }
    if selected("bnr") {
        futures.push(Box::pin(bnr::discover(client)));
    }
    if selected("btrc") {
        futures.push(Box::pin(btrc::discover(client)));
    }
    if selected("cbc") {
        futures.push(Box::pin(cbc::discover(client)));
    }
    if selected("cesky-rozhlas") {
        futures.push(Box::pin(cesky_rozhlas::discover(client)));
    }
    if selected("curated") {
        futures.push(Box::pin(curated::discover(client)));
    }
    if selected("dr") {
        futures.push(Box::pin(dr::discover(client)));
    }
    if selected("ebc") {
        futures.push(Box::pin(ebc::discover(client)));
    }
    if selected("err") {
        futures.push(Box::pin(err::discover(client)));
    }
    if selected("ert") {
        futures.push(Box::pin(ert::discover(client)));
    }
    if selected("fluxfm") {
        futures.push(Box::pin(fluxfm::discover(client)));
    }
    if selected("global") {
        futures.push(Box::pin(global::discover(client)));
    }
    if selected("hrt") {
        futures.push(Box::pin(hrt::discover(client)));
    }
    if selected("icecast-yp") {
        futures.push(Box::pin(icecast_yp::discover(client)));
    }
    if selected("irtp") {
        futures.push(Box::pin(irtp::discover(client)));
    }
    if selected("latvijas-radio") {
        futures.push(Box::pin(latvijas_radio::discover(client)));
    }
    if selected("lrt") {
        futures.push(Box::pin(lrt::discover(client)));
    }
    if selected("mrt") {
        futures.push(Box::pin(mrt::discover(client)));
    }
    if selected("mtva") {
        futures.push(Box::pin(mtva::discover(client)));
    }
    if selected("ncn") {
        futures.push(Box::pin(ncn::discover(client)));
    }
    if selected("npo") {
        futures.push(Box::pin(npo::discover(client)));
    }
    if selected("nrk") {
        futures.push(Box::pin(nrk::discover(client)));
    }
    if selected("orf") {
        futures.push(Box::pin(orf::discover(client)));
    }
    if selected("polskie-radio") {
        futures.push(Box::pin(polskie_radio::discover(client)));
    }
    if selected("publica-fm") {
        futures.push(Box::pin(publica_fm::discover(client)));
    }
    if selected("radio-browser") {
        futures.push(Box::pin(radio_browser::discover(client)));
    }
    if selected("radio-france") {
        futures.push(Box::pin(radio_france::discover(client)));
    }
    if selected("radio-nacional") {
        futures.push(Box::pin(radio_nacional::discover(client)));
    }
    if selected("radio-paradise") {
        futures.push(Box::pin(radio_paradise::discover(client)));
    }
    if selected("radio-romania") {
        futures.push(Box::pin(radio_romania::discover(client)));
    }
    if selected("rai") {
        futures.push(Box::pin(rai::discover(client)));
    }
    if selected("rik") {
        futures.push(Box::pin(rik::discover(client)));
    }
    if selected("rinse") {
        futures.push(Box::pin(rinse::discover(client)));
    }
    if selected("rnp") {
        futures.push(Box::pin(rnp::discover(client)));
    }
    if selected("rnu") {
        futures.push(Box::pin(rnu::discover(client)));
    }
    if selected("rnv") {
        futures.push(Box::pin(rnv::discover(client)));
    }
    if selected("rtbf") {
        futures.push(Box::pin(rtbf::discover(client)));
    }
    if selected("rte") {
        futures.push(Box::pin(rte::discover(client)));
    }
    if selected("rtk") {
        futures.push(Box::pin(rtk::discover(client)));
    }
    if selected("rtl-lu") {
        futures.push(Box::pin(rtl_lu::discover(client)));
    }
    if selected("rtp") {
        futures.push(Box::pin(rtp::discover(client)));
    }
    if selected("rts") {
        futures.push(Box::pin(rts::discover(client)));
    }
    if selected("rtsh") {
        futures.push(Box::pin(rtsh::discover(client)));
    }
    if selected("rtva") {
        futures.push(Box::pin(rtva::discover(client)));
    }
    if selected("rtvc") {
        futures.push(Box::pin(rtvc::discover(client)));
    }
    if selected("rtve") {
        futures.push(Box::pin(rtve::discover(client)));
    }
    if selected("rtvslo") {
        futures.push(Box::pin(rtvslo::discover(client)));
    }
    if selected("ruv") {
        futures.push(Box::pin(ruv::discover(client)));
    }
    if selected("sbs") {
        futures.push(Box::pin(sbs::discover(client)));
    }
    if selected("somafm") {
        futures.push(Box::pin(somafm::discover(client)));
    }
    if selected("sr") {
        futures.push(Box::pin(sr::discover(client)));
    }
    if selected("srgssr") {
        futures.push(Box::pin(srgssr::discover(client)));
    }
    if selected("srs") {
        futures.push(Box::pin(srs::discover(client)));
    }
    if selected("stvr") {
        futures.push(Box::pin(stvr::discover(client)));
    }
    if selected("suspilne") {
        futures.push(Box::pin(suspilne::discover(client)));
    }
    if selected("trm") {
        futures.push(Box::pin(trm::discover(client)));
    }
    if selected("vatican-radio") {
        futures.push(Box::pin(vatican_radio::discover(client)));
    }
    if selected("vgtrk") {
        futures.push(Box::pin(vgtrk::discover(client)));
    }
    if selected("wireless") {
        futures.push(Box::pin(wireless::discover(client)));
    }
    if selected("yle") {
        futures.push(Box::pin(yle::discover(client)));
    }

    let all: Vec<_> = futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect();
    info!(total = all.len(), "All providers complete");
    all
}

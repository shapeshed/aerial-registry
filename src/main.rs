mod curation;
mod http;
mod pipeline;
mod providers;
mod station;

use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let client = http::build_client()?;

    if std::env::args().nth(1).as_deref() == Some("prune-curated") {
        return curation::prune_curated(&client).await;
    }

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
        rtbf,
        rte,
        rtp,
        rtve,
        sbs,
        sr,
        wireless,
    ) = tokio::join!(
        providers::abc::discover(&client),
        providers::ard::discover(&client),
        providers::bbc::discover(&client),
        providers::bauer::discover(&client),
        providers::cbc::discover(&client),
        providers::curated::discover(&client),
        providers::dr::discover(&client),
        providers::global::discover(&client),
        providers::npo::discover(&client),
        providers::nrk::discover(&client),
        providers::orf::discover(&client),
        providers::radio_france::discover(&client),
        providers::rai::discover(&client),
        providers::rtbf::discover(&client),
        providers::rte::discover(&client),
        providers::rtp::discover(&client),
        providers::rtve::discover(&client),
        providers::sbs::discover(&client),
        providers::sr::discover(&client),
        providers::wireless::discover(&client),
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

    let deduped = pipeline::dedup::dedup(all);
    let enriched = pipeline::enrich::enrich(&client, deduped).await;
    let live = pipeline::liveness::check(&client, enriched).await;
    pipeline::output::write(live)?;

    Ok(())
}

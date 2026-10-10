mod config;
mod curation;
mod http;
mod pipeline;
mod providers;
mod radio_browser_client;
mod scan;
mod station;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configuration up front so a malformed file fails fast, before any
    // network work begins.
    config::get();

    let client = http::build_client()?;

    if let Some("prune-curated") = std::env::args().nth(1).as_deref() {
        return curation::prune_curated(&client).await;
    }

    if let Some("icecast-scan") = std::env::args().nth(1).as_deref() {
        return scan::run(&client, std::env::args().nth(2).as_deref()).await;
    }

    let all = providers::discover_all(&client).await;
    let deduped = pipeline::dedup::dedup(all);
    let enriched = pipeline::enrich::enrich(&client, deduped).await;
    let overlaid = pipeline::overlay::apply(enriched);
    let live = pipeline::liveness::check(&client, overlaid).await;
    let previous = pipeline::guard::load_from_config();
    let (guarded, interventions) = pipeline::guard::apply(live, previous.as_deref());
    pipeline::report::write(previous.as_deref(), &guarded, &interventions);
    pipeline::output::write(guarded)?;

    Ok(())
}

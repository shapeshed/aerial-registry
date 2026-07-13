mod curation;
mod http;
mod pipeline;
mod providers;
mod station;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let client = http::build_client()?;

    match std::env::args().nth(1).as_deref() {
        Some("prune-curated") => return curation::prune_curated(&client).await,
        Some("enrich-overlay") => return pipeline::overlay::build(&client).await,
        _ => {}
    }

    let all = providers::discover_all(&client).await;
    let deduped = pipeline::dedup::dedup(all);
    let enriched = pipeline::enrich::enrich(&client, deduped).await;
    let overlaid = pipeline::overlay::apply(enriched);
    let live = pipeline::liveness::check(&client, overlaid).await;
    pipeline::output::write(live)?;

    Ok(())
}

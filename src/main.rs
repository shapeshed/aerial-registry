mod config;
mod curation;
mod http;
mod pipeline;
mod providers;
mod radio_browser_client;
mod scan;
mod station;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Build the Aerial station registry, or run the discovery tooling around it.
#[derive(Parser)]
#[command(name = "aerial-registry", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Build the registry from every provider. This is the default.
    Build,
    /// Remove curated stations whose streams no longer resolve.
    PruneCurated,
    /// Internet-wide Icecast host discovery and enumeration.
    IcecastScan {
        /// Source: yp | netlas | file | shodan | censys.
        source: Option<String>,
        /// Source-specific argument, e.g. the host-list path for `file`.
        arg: Option<String>,
        /// Write every responding host as `host<TAB>mount name`, one line per
        /// mount. This is the machine-readable output the pipeline and the
        /// scan measurements use, so callers never re-implement enumeration.
        #[arg(long, value_name = "PATH")]
        responders: Option<PathBuf>,
    },
}

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

    match Cli::parse().command.unwrap_or(Command::Build) {
        Command::Build => build(&client).await,
        Command::PruneCurated => curation::prune_curated(&client).await,
        Command::IcecastScan {
            source,
            arg,
            responders,
        } => {
            scan::run(
                &client,
                source.as_deref(),
                arg.as_deref(),
                responders.as_deref(),
            )
            .await
        }
    }
}

/// The provider → dedup → enrich → overlay → liveness → output pipeline.
async fn build(client: &http::Client) -> anyhow::Result<()> {
    let all = providers::discover_all(client).await;
    let deduped = pipeline::dedup::dedup(all);
    let enriched = pipeline::enrich::enrich(client, deduped).await;
    let overlaid = pipeline::overlay::apply(enriched);
    let live = pipeline::liveness::check(client, overlaid).await;
    let previous = pipeline::guard::load_from_config();
    let (guarded, interventions) = pipeline::guard::apply(live, previous.as_deref());
    pipeline::report::write(previous.as_deref(), &guarded, &interventions);
    pipeline::output::write(guarded)?;
    Ok(())
}

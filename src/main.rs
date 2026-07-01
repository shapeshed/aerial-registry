mod curation;
mod http;
mod pipeline;
mod providers;
mod station;

use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Ok(sample) = std::env::var("AI_ASSESSMENT_SAMPLE") {
        match serde_json::from_str::<pipeline::ai::AiAssessment>(&sample) {
            Ok(assessment) => {
                let issues = pipeline::ai::validate(&assessment);
                info!(issues = issues.len(), "AI assessment sample validated");
            }
            Err(e) => warn!(error = %e, "AI assessment sample was not valid JSON"),
        }
    }

    let client = http::build_client()?;

    if std::env::args().nth(1).as_deref() == Some("prune-curated") {
        return curation::prune_curated(&client).await;
    }

    info!("Starting provider discovery");

    let enabled = enabled_providers();
    let (ard, bbc, bauer, curated, global, nrj_audio, radio_browser, radio_france, rai, rte, rtve, wireless) = tokio::join!(
        async {
            if provider_enabled(&enabled, "ard") {
                providers::ard::discover(&client).await
            } else {
                disabled("ard")
            }
        },
        async {
            if provider_enabled(&enabled, "bbc") {
                providers::bbc::discover(&client).await
            } else {
                disabled("bbc")
            }
        },
        async {
            if provider_enabled(&enabled, "bauer") {
                providers::bauer::discover(&client).await
            } else {
                disabled("bauer")
            }
        },
        async {
            if provider_enabled(&enabled, "curated") {
                providers::curated::discover(&client).await
            } else {
                disabled("curated")
            }
        },
        async {
            if provider_enabled(&enabled, "global") {
                providers::global::discover(&client).await
            } else {
                disabled("global")
            }
        },
        async {
            if provider_enabled(&enabled, "nrj-audio") {
                providers::nrj_audio::discover(&client).await
            } else {
                disabled("nrj-audio")
            }
        },
        async {
            if provider_enabled(&enabled, "radio-browser") {
                providers::radio_browser::discover(&client).await
            } else {
                disabled("radio-browser")
            }
        },
        async {
            if provider_enabled(&enabled, "radio-france") {
                providers::radio_france::discover(&client).await
            } else {
                disabled("radio-france")
            }
        },
        async {
            if provider_enabled(&enabled, "rai") {
                providers::rai::discover(&client).await
            } else {
                disabled("rai")
            }
        },
        async {
            if provider_enabled(&enabled, "rte") {
                providers::rte::discover(&client).await
            } else {
                disabled("rte")
            }
        },
        async {
            if provider_enabled(&enabled, "rtve") {
                providers::rtve::discover(&client).await
            } else {
                disabled("rtve")
            }
        },
        async {
            if provider_enabled(&enabled, "wireless") {
                providers::wireless::discover(&client).await
            } else {
                disabled("wireless")
            }
        },
    );

    let all: Vec<_> = [
        ard,
        bbc,
        bauer,
        curated,
        global,
        nrj_audio,
        radio_browser,
        radio_france,
        rai,
        rte,
        rtve,
        wireless,
    ]
    .into_iter()
    .flatten()
    .collect();
    info!(total = all.len(), "All providers complete");

    let quality_results = pipeline::quality::assess(all);

    match pipeline::cache::Cache::open("state/registry.sqlite") {
        Ok(cache) => {
            let mut changed = 0usize;
            for result in &quality_results {
                match cache.changed(&result.station) {
                    Ok(true) => changed += 1,
                    Ok(false) => {}
                    Err(e) => warn!(error = %e, "Failed to read cache entry"),
                }
                if let Err(e) = cache.record_source(&result.station) {
                    warn!(error = %e, "Failed to write cache entry");
                }
            }
            info!(
                changed,
                total = quality_results.len(),
                "Source cache updated"
            );
        }
        Err(e) => warn!(error = %e, "Source cache unavailable"),
    }

    let quality_checked = pipeline::quality::accepted_stations(quality_results);
    let ai_enriched = pipeline::ai::enrich(&client, quality_checked).await;
    let ai_checked = pipeline::quality::accepted_stations(pipeline::quality::assess(ai_enriched));
    let deduped = pipeline::dedup::dedup(ai_checked);
    let live = if env_flag("AERIAL_SKIP_LIVENESS") {
        info!("Liveness checks skipped because AERIAL_SKIP_LIVENESS is enabled");
        deduped
    } else {
        pipeline::liveness::check(&client, deduped).await
    };
    pipeline::output::write(live)?;

    Ok(())
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn enabled_providers() -> Option<std::collections::HashSet<String>> {
    std::env::var("AERIAL_PROVIDERS").ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|provider| !provider.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn provider_enabled(enabled: &Option<std::collections::HashSet<String>>, provider: &str) -> bool {
    enabled
        .as_ref()
        .is_none_or(|enabled| enabled.contains(provider))
}

fn disabled(provider: &str) -> Vec<station::Station> {
    info!(provider, "Provider disabled by AERIAL_PROVIDERS");
    Vec::new()
}

use std::{collections::HashSet, fs, sync::Arc};

use serde::Deserialize;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::info;

use crate::pipeline::liveness;

const STATIONS_PATH: &str = "stations.toml";
const REJECTED_PATH: &str = "stations_rejected.toml";
const MAX_CONCURRENT: usize = 50;

#[derive(Clone)]
struct StationBlock {
    index: usize,
    block: String,
    name: String,
    stream_url: String,
}

struct PruneResult {
    station: StationBlock,
    keep: bool,
    stream_url: String,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct StationFile {
    stations: Vec<TomlStation>,
}

#[derive(Deserialize)]
struct TomlStation {
    name: String,
    stream_url: String,
}

#[derive(Deserialize, Default)]
struct RejectedFile {
    rejected: Vec<RejectedStation>,
}

#[derive(Deserialize)]
struct RejectedStation {
    stream_url: String,
}

pub async fn prune_curated(client: &reqwest::Client) -> anyhow::Result<()> {
    let source = fs::read_to_string(STATIONS_PATH)?;
    let (header, stations) = parse_station_blocks(&source)?;
    let existing_rejected = load_rejected_urls();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut tasks = JoinSet::new();
    for station in stations {
        let client = client.clone();
        let semaphore = semaphore.clone();
        tasks.spawn(async move {
            let _permit = semaphore.acquire().await.expect("semaphore is not closed");
            match liveness::validate_imported_stream_url(&client, &station.stream_url).await {
                Ok(stream_url) => PruneResult {
                    station,
                    keep: true,
                    stream_url,
                    reason: None,
                },
                Err(reason) => {
                    let message = reason.message().to_string();
                    PruneResult {
                        stream_url: station.stream_url.clone(),
                        station,
                        keep: false,
                        reason: Some(message),
                    }
                }
            }
        });
    }

    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result?);
    }
    results.sort_by_key(|result| result.station.index);

    let kept = results.iter().filter(|result| result.keep).count();
    let upgraded = results
        .iter()
        .filter(|result| result.keep && result.stream_url != result.station.stream_url)
        .count();
    let rejected: Vec<_> = results.iter().filter(|result| !result.keep).collect();

    info!(
        total = results.len(),
        kept,
        upgraded,
        rejected = rejected.len(),
        "Curated station pruning complete"
    );

    if rejected.is_empty() && upgraded == 0 {
        return Ok(());
    }

    let blocks: Vec<_> = results
        .iter()
        .filter(|result| result.keep)
        .map(|result| {
            if result.stream_url == result.station.stream_url {
                result.station.block.clone()
            } else {
                replace_stream_url(
                    &result.station.block,
                    &result.station.stream_url,
                    &result.stream_url,
                )
            }
        })
        .collect();

    fs::write(
        STATIONS_PATH,
        format!("{}\n\n{}\n", header, blocks.join("\n\n")),
    )?;
    append_rejections(&rejected, &existing_rejected)?;

    Ok(())
}

fn parse_station_blocks(source: &str) -> anyhow::Result<(String, Vec<StationBlock>)> {
    let Some(first_station) = source.find("[[stations]]") else {
        return Ok((source.trim_end().to_string(), Vec::new()));
    };

    let header = source[..first_station].trim_end().to_string();
    let body = &source[first_station..];
    let mut stations = Vec::new();

    for (index, part) in body.split("\n[[stations]]").enumerate() {
        let block = if index == 0 {
            part.trim().to_string()
        } else {
            format!("[[stations]]\n{}", part.trim())
        };
        let parsed: StationFile = toml::from_str(&block)?;
        let Some(station) = parsed.stations.into_iter().next() else {
            continue;
        };
        stations.push(StationBlock {
            index,
            block,
            name: station.name,
            stream_url: station.stream_url,
        });
    }

    Ok((header, stations))
}

fn replace_stream_url(block: &str, old: &str, new: &str) -> String {
    block.replacen(
        &format!("stream_url = {}", toml_string(old)),
        &format!("stream_url = {}", toml_string(new)),
        1,
    )
}

fn append_rejections(results: &[&PruneResult], existing: &HashSet<String>) -> anyhow::Result<()> {
    let mut entries = String::new();

    for result in results {
        if existing.contains(&normalise_url(&result.stream_url)) {
            continue;
        }
        entries.push_str("\n[[rejected]]\n");
        entries.push_str(&format!(
            "stream_url = {}\n",
            toml_string(&result.stream_url)
        ));
        entries.push_str(&format!("name = {}\n", toml_string(&result.station.name)));
        entries.push_str(&format!(
            "reason = {}\n",
            toml_string(
                result
                    .reason
                    .as_deref()
                    .unwrap_or("Removed by liveness pruning.")
            )
        ));
    }

    if !entries.is_empty() {
        let mut rejected = fs::read_to_string(REJECTED_PATH).unwrap_or_default();
        if !rejected.ends_with('\n') {
            rejected.push('\n');
        }
        rejected.push_str(&entries);
        fs::write(REJECTED_PATH, rejected)?;
    }

    Ok(())
}

fn load_rejected_urls() -> HashSet<String> {
    let Ok(source) = fs::read_to_string(REJECTED_PATH) else {
        return HashSet::new();
    };
    let parsed: RejectedFile = toml::from_str(&source).unwrap_or_default();
    parsed
        .rejected
        .into_iter()
        .map(|station| normalise_url(&station.stream_url))
        .collect()
}

fn normalise_url(url: &str) -> String {
    url.to_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

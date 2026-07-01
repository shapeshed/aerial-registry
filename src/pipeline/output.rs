use std::io::Write;

use flate2::{Compression, write::GzEncoder};
use tracing::info;

use crate::station::Station;

const OUTPUT_PATH: &str = "registry.json.gz";

pub fn write(mut stations: Vec<Station>) -> anyhow::Result<()> {
    stations.sort_by(|a, b| a.name.cmp(&b.name));

    let json = serde_json::to_vec_pretty(&stations)?;

    let file = std::fs::File::create(OUTPUT_PATH)?;
    let mut gz = GzEncoder::new(file, Compression::best());
    gz.write_all(&json)?;
    gz.finish()?;

    let size = std::fs::metadata(OUTPUT_PATH)?.len();
    info!(
        path = OUTPUT_PATH,
        stations = stations.len(),
        size_bytes = size,
        "Registry written"
    );
    Ok(())
}

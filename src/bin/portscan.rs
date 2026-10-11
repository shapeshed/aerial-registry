//! Standalone TCP connect scanner.
//!
//! This binary is deliberately **independent of the registry pipeline**. Its
//! only job is to turn a set of IP targets and ports into a list of `ip:port`
//! lines. `aerial-registry icecast-scan file <path>` then enumerates that list
//! (asking each host for `/status-json.xsl` and extracting its mounts), so the
//! two halves compose without either knowing about the other.
//!
//! ```text
//! portscan --targets targets.txt --append --out icecast-hosts.txt
//! ```
//!
//! Targets are IPv4 addresses, CIDR blocks or ranges (`a.b.c.d-a.b.c.d`), one
//! per line in a file or as positional arguments; `#` starts a comment.
//!
//! # Before you scan anything
//!
//! Only scan ranges you are authorised to scan. Internet-wide port scanning is
//! legal in many jurisdictions but routinely breaches hosting providers' terms
//! of service, and a fast scan looks exactly like an attack. Prefer the
//! ASNs/prefixes already known to host radio servers over blind address space,
//! keep `--concurrency` modest, and expect abuse reports if you do not.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use clap::Parser;
use futures::StreamExt;
use futures::stream;
use tokio::net::TcpStream;
use tracing::{info, warn};

/// Ports that Icecast/Shoutcast and the panels around them commonly listen on.
const DEFAULT_PORTS: &str = "8000,8001,8008,8010,8080,8081,8443,8888,9000";

const DEFAULT_CONCURRENCY: usize = 1024;
const DEFAULT_TIMEOUT_MS: u64 = 3000;

/// TCP connect scanner that emits `ip:port` lines for open ports.
#[derive(Parser, Debug)]
#[command(
    name = "portscan",
    about = "TCP connect scanner that emits `ip:port` lines",
    after_help = "EXAMPLES:\n  \
        portscan 10.0.0.0/24 -p 8000 -o hosts.txt\n  \
        portscan --targets prefixes.txt --append -o icecast-hosts.txt\n  \
        portscan --targets 203.0.113.0/24 --ports 8000-8010"
)]
struct Cli {
    /// IP, CIDR, range, or a file containing one per line (repeatable).
    #[arg(short = 't', long = "targets", value_name = "SPEC")]
    targets: Vec<String>,

    /// IP, CIDR, range, or a file containing one per line.
    #[arg(value_name = "TARGET")]
    positional: Vec<String>,

    /// Ports to scan: comma-separated list or ranges, e.g. 8000,8080 or 8000-8010.
    #[arg(
        short = 'p',
        long = "ports",
        value_name = "SPEC",
        default_value = DEFAULT_PORTS
    )]
    ports: Ports,

    /// Write hosts to a file instead of stdout.
    #[arg(short = 'o', long = "out", value_name = "PATH")]
    out: Option<PathBuf>,

    /// Merge into the output file instead of replacing it.
    #[arg(long)]
    append: bool,

    /// Concurrent connections.
    #[arg(short = 'c', long, value_name = "N", default_value_t = DEFAULT_CONCURRENCY)]
    concurrency: usize,

    /// Connect timeout, in milliseconds.
    #[arg(long, value_name = "MS", default_value_t = DEFAULT_TIMEOUT_MS)]
    timeout: u64,
}

/// A validated, sorted set of ports.
#[derive(Clone, Debug)]
struct Ports(Vec<u16>);

impl FromStr for Ports {
    type Err = String;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let mut ports = BTreeSet::new();
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('-') {
                Some((start, end)) => {
                    let start: u16 = start
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid port: {start}"))?;
                    let end: u16 = end
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid port: {end}"))?;
                    if start > end {
                        return Err(format!("invalid port range: {part}"));
                    }
                    ports.extend(start..=end);
                }
                None => {
                    ports.insert(part.parse().map_err(|_| format!("invalid port: {part}"))?);
                }
            }
        }
        if ports.is_empty() {
            return Err("no ports given".to_string());
        }
        Ok(Ports(ports.into_iter().collect()))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let timeout = Duration::from_millis(cli.timeout);

    let specs: Vec<String> = cli
        .targets
        .iter()
        .chain(cli.positional.iter())
        .cloned()
        .collect();
    let targets = collect_targets(&specs)?;
    if targets.is_empty() {
        warn!("No targets given — nothing to scan (use --targets or positional CIDRs)");
        return Ok(());
    }

    let addresses: u64 = targets
        .iter()
        .map(|(start, end)| u64::from(end - start) + 1)
        .sum();
    info!(
        ranges = targets.len(),
        addresses,
        ports = ?cli.ports.0,
        probes = addresses * cli.ports.0.len() as u64,
        concurrency = cli.concurrency,
        "Scanning"
    );

    let partial = cli.out.as_ref().map(|path| partial_path(path));
    let mut sink = match &partial {
        Some(path) => Some(BufWriter::new(File::create(path)?)),
        None => None,
    };

    let found = scan(
        &targets,
        &cli.ports.0,
        cli.concurrency,
        timeout,
        sink.as_mut(),
    )
    .await?;
    drop(sink);

    let lines: Vec<String> = found
        .iter()
        .map(|(ip, port)| format!("{}:{port}", Ipv4Addr::from(*ip)))
        .collect();

    info!(open = lines.len(), "Scan complete");

    match &cli.out {
        Some(path) => {
            let path = path.to_string_lossy();
            let added = write_hosts(&path, &lines, cli.append)?;
            if let Some(partial) = &partial {
                let _ = std::fs::remove_file(partial);
            }
            info!(path = %path, added, total = read_hosts(&path).len(), "Hosts written");
        }
        None => {
            for line in &lines {
                println!("{line}");
            }
        }
    }
    Ok(())
}

/// The sidecar file results are streamed to, so an interrupted multi-hour scan
/// is not lost. Removed once the sorted output has been written.
fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

/// Connect to every `(ip, port)` pair, returning the ones that accept.
///
/// Results are written to `sink` as they arrive rather than only at the end: a
/// scan is long enough that losing it to an interruption is the dominant risk.
async fn scan(
    targets: &[(u32, u32)],
    ports: &[u16],
    concurrency: usize,
    timeout: Duration,
    mut sink: Option<&mut BufWriter<File>>,
) -> std::io::Result<Vec<(u32, u16)>> {
    let progress = Arc::new(AtomicUsize::new(0));
    let ips = targets.iter().copied().flat_map(|(start, end)| start..=end);
    let pairs = ips.flat_map(|ip| ports.iter().copied().map(move |port| (ip, port)));

    let mut stream = Box::pin(
        stream::iter(pairs)
            .map(|(ip, port)| {
                let progress = Arc::clone(&progress);
                async move {
                    let open = is_open(ip, port, timeout).await;
                    let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
                    if done % 200_000 == 0 {
                        info!(scanned = done, "Progress");
                    }
                    open.then_some((ip, port))
                }
            })
            .buffer_unordered(concurrency)
            .filter_map(|found| async move { found }),
    );

    let mut open = Vec::new();
    while let Some((ip, port)) = stream.next().await {
        open.push((ip, port));
        if let Some(sink) = sink.as_mut() {
            writeln!(sink, "{}:{}", Ipv4Addr::from(ip), port)?;
        }
    }
    if let Some(sink) = sink.as_mut() {
        sink.flush()?;
    }
    Ok(open)
}

async fn is_open(ip: u32, port: u16, timeout: Duration) -> bool {
    let address = (Ipv4Addr::from(ip), port);
    matches!(
        tokio::time::timeout(timeout, TcpStream::connect(address)).await,
        Ok(Ok(_))
    )
}

/// Expand every target spec into inclusive `(start, end)` IPv4 ranges.
fn collect_targets(specs: &[String]) -> anyhow::Result<Vec<(u32, u32)>> {
    let mut specs = specs.to_vec();

    // Read any spec that names an existing file, so both `--targets list.txt`
    // and `--targets 10.0.0.0/8` work.
    let mut ranges = Vec::new();
    while let Some(spec) = specs.pop() {
        let path = std::path::Path::new(&spec);
        if path.is_file() {
            let text = std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("cannot read targets {spec}: {e}"))?;
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if !line.is_empty() {
                    specs.push(line.to_string());
                }
            }
        } else {
            ranges.push(parse_target(&spec)?);
        }
    }
    Ok(ranges)
}

/// Parse `1.2.3.4`, `1.2.3.0/24` or `1.2.3.4-1.2.3.10` into a range.
fn parse_target(spec: &str) -> anyhow::Result<(u32, u32)> {
    if let Some((base, bits)) = spec.split_once('/') {
        let ip = parse_ip(base)?;
        let bits: u32 = bits
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid prefix length in {spec}"))?;
        if bits > 32 {
            anyhow::bail!("prefix length out of range in {spec}");
        }
        let mask = if bits == 0 {
            0
        } else {
            u32::MAX << (32 - bits)
        };
        let start = ip & mask;
        let end = start | !mask;
        return Ok((start, end));
    }
    if let Some((start, end)) = spec.split_once('-') {
        return Ok((parse_ip(start)?, parse_ip(end)?));
    }
    let ip = parse_ip(spec)?;
    Ok((ip, ip))
}

fn parse_ip(text: &str) -> anyhow::Result<u32> {
    let ip: Ipv4Addr = text
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid IPv4 address: {text}"))?;
    Ok(u32::from(ip))
}

/// Merge `lines` into the host file at `path` (union, sorted) when `append`,
/// otherwise replace it. Returns how many entries were newly added.
fn write_hosts(path: &str, lines: &[String], append: bool) -> anyhow::Result<usize> {
    let existing = if append { read_hosts(path) } else { Vec::new() };
    let before = existing.len();

    let mut set: BTreeSet<String> = existing.into_iter().collect();
    set.extend(lines.iter().cloned());

    let body = set.into_iter().collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(path, body).map_err(|e| anyhow::anyhow!("cannot write {path}: {e}"))?;
    Ok(read_hosts(path).len().saturating_sub(before))
}

fn read_hosts(path: &str) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_cidr_and_range_targets() {
        assert_eq!(parse_target("1.2.3.4").unwrap(), (0x01020304, 0x01020304));
        assert_eq!(
            parse_target("10.0.0.0/30").unwrap(),
            (0x0a000000, 0x0a000003)
        );
        assert_eq!(parse_target("10.0.0.0/0").unwrap(), (0, u32::MAX));
        assert_eq!(
            parse_target("192.168.1.1-192.168.1.3").unwrap(),
            (0xc0a80101, 0xc0a80103)
        );
    }

    #[test]
    fn rejects_bad_targets() {
        assert!(parse_target("300.1.1.1").is_err());
        assert!(parse_target("10.0.0.0/33").is_err());
    }

    #[test]
    fn parses_port_lists_and_ranges() {
        assert_eq!(Ports::from_str("8000,8080").unwrap().0, vec![8000, 8080]);
        assert_eq!(
            Ports::from_str("8000-8002,9000").unwrap().0,
            vec![8000, 8001, 8002, 9000]
        );
        assert_eq!(Ports::from_str("8080,8000").unwrap().0, vec![8000, 8080]);
        assert!(Ports::from_str("9000-8000").is_err());
        assert!(Ports::from_str("").is_err());
    }

    #[test]
    fn cli_parses_flags_and_positional_targets() {
        let cli = Cli::try_parse_from([
            "portscan",
            "10.0.0.0/24",
            "--targets",
            "prefixes.txt",
            "-p",
            "8000,8080",
            "-o",
            "hosts.txt",
            "--append",
            "-c",
            "64",
        ])
        .unwrap();
        assert_eq!(cli.positional, vec!["10.0.0.0/24"]);
        assert_eq!(cli.targets, vec!["prefixes.txt"]);
        assert_eq!(cli.ports.0, vec![8000, 8080]);
        assert_eq!(cli.out, Some(PathBuf::from("hosts.txt")));
        assert!(cli.append);
        assert_eq!(cli.concurrency, 64);
    }
}

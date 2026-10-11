//! Turn a list of known radio hosts into the address space worth scanning.
//!
//! This is the target-generation half of the discovery strategy in
//! `docs/scanning.md`. Given hosts we already know (from Radio-Browser
//! enumeration, Xiph YP, Netlas, previous scans), it:
//!
//! 1. resolves each IP to its ASN using a local `iptoasn` table,
//! 2. ranks ASNs by how many known radio hosts they contain,
//! 3. fetches the announced prefixes of the top ASNs from RIPE Stat,
//! 4. writes those prefixes for `portscan --targets`.
//!
//! The premise is that Icecast servers cluster in a small number of hosting
//! providers, so scanning the prefixes of radio-heavy ASNs beats sampling the
//! whole internet by orders of magnitude.
//!
//! ```text
//! targets --hosts icecast-hosts.txt --top 20 --out prefixes.txt
//! portscan --targets prefixes.txt --append --out icecast-hosts.txt
//! aerial-registry icecast-scan file icecast-hosts.txt
//! ```

use std::collections::HashMap;
use std::io::Read;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use futures::StreamExt;
use futures::stream;
use serde::Deserialize;
use tracing::{info, warn};

const ASN_TABLE_URL: &str = "https://iptoasn.com/data/ip2asn-v4.tsv.gz";
const RIPE_PREFIXES_URL: &str = "https://stat.ripe.net/data/announced-prefixes/data.json";
const DEFAULT_HOSTS: &str = "icecast-hosts.txt";
/// The `iptoasn` dump is regenerated daily; a week is plenty fresh.
const TABLE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Expand known radio hosts into the ASN prefixes worth scanning.
#[derive(Parser, Debug)]
#[command(
    name = "targets",
    about = "Expand known radio hosts into ASN prefixes to scan",
    after_help = "EXAMPLES:\n  \
        targets --hosts icecast-hosts.txt --top 20 --dry-run\n  \
        targets --top 20 --out prefixes.txt\n  \
        portscan --targets prefixes.txt --append --out icecast-hosts.txt"
)]
struct Cli {
    /// Host list(s): `ip` or `ip:port` per line, `#` comments allowed.
    #[arg(short = 'H', long = "hosts", value_name = "PATH", default_value = DEFAULT_HOSTS)]
    hosts: Vec<String>,

    /// Keep the N ASNs holding the most known hosts.
    #[arg(short = 'n', long = "top", value_name = "N", default_value_t = 50)]
    top: usize,

    /// Ignore ASNs with fewer than this many known hosts.
    #[arg(long, value_name = "N", default_value_t = 2)]
    min_hosts: usize,

    /// Stop adding ASNs once their prefixes exceed this many addresses.
    /// Selection is by density, so cheap dense ASNs are picked first.
    #[arg(long, value_name = "N", default_value_t = 5_000_000)]
    max_addresses: u64,

    /// Cached `ip2asn` table (downloaded when missing or stale).
    #[arg(long, value_name = "PATH")]
    asn_table: Option<PathBuf>,

    /// Re-download the ASN table even if the cache is fresh.
    #[arg(long)]
    refresh: bool,

    /// Print the ASN ranking and stop, without fetching prefixes.
    #[arg(long)]
    dry_run: bool,

    /// Write prefixes here instead of stdout.
    #[arg(short = 'o', long = "out", value_name = "PATH")]
    out: Option<PathBuf>,

    /// RIPE Stat requests in flight.
    #[arg(short = 'c', long, value_name = "N", default_value_t = 4)]
    concurrency: usize,
}

/// One `iptoasn` range: `start  end  asn  country  description`.
#[derive(Debug, Clone)]
struct AsnRange {
    start: u32,
    end: u32,
    asn: u32,
    description: String,
}

/// An ASN that holds known radio hosts, together with the prefixes it announces.
struct Candidate {
    asn: u32,
    hosts: usize,
    description: String,
    prefixes: Vec<String>,
}

impl Candidate {
    fn addresses(&self) -> u64 {
        self.prefixes
            .iter()
            .filter_map(|prefix| prefix_addresses(prefix))
            .sum()
    }

    /// Known hosts per announced address — the higher the better.
    fn density(&self) -> f64 {
        match self.addresses() {
            0 => 0.0,
            addresses => self.hosts as f64 / addresses as f64,
        }
    }
}

/// Render density as "1 per N addresses", which is easier to reason about.
fn density_label(candidate: &Candidate) -> String {
    let density = candidate.density();
    if density <= 0.0 {
        "n/a".to_string()
    } else {
        format!("1/{}", (1.0 / density).round() as u64)
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
    let client = reqwest::Client::builder()
        .user_agent(concat!("aerial-registry/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .build()?;

    let ips = read_hosts(&cli.hosts)?;
    if ips.is_empty() {
        warn!("No usable hosts found — nothing to expand");
        return Ok(());
    }
    info!(hosts = ips.len(), "Hosts loaded");

    let table_path = cli.asn_table.clone().unwrap_or_else(default_table_path);
    let table = load_asn_table(&client, &table_path, cli.refresh).await?;
    info!(ranges = table.len(), path = %table_path.display(), "ASN table loaded");

    let ranking = rank_asns(&table, &ips, cli.min_hosts);
    if ranking.is_empty() {
        warn!("No ASNs matched — are the hosts routable IPv4 addresses?");
        return Ok(());
    }

    println!("{:<10} {:>7}  {}", "ASN", "hosts", "description");
    for (asn, count, description) in ranking.iter().take(cli.top) {
        println!("AS{asn:<8} {count:>7}  {description}");
    }

    if cli.dry_run {
        info!(asns = ranking.len(), "Dry run — not fetching prefixes");
        return Ok(());
    }

    let considered: Vec<(u32, usize, String)> = ranking.iter().take(cli.top).cloned().collect();
    let asns: Vec<u32> = considered.iter().map(|(asn, ..)| *asn).collect();
    info!(asns = asns.len(), "Fetching announced prefixes");
    let fetched = fetch_prefixes(&client, &asns, cli.concurrency).await;

    let mut candidates: Vec<Candidate> = considered
        .into_iter()
        .map(|(asn, hosts, description)| {
            let prefixes = fetched
                .iter()
                .find(|(fetched_asn, _)| *fetched_asn == asn)
                .map(|(_, prefixes)| prefixes.clone())
                .unwrap_or_default();
            Candidate {
                asn,
                hosts,
                description,
                prefixes,
            }
        })
        .collect();

    // Rank by density: known hosts per announced address. A broadcaster ASN
    // with three hosts in a /22 beats a cloud ASN with thirty in a /11.
    candidates.sort_by(|a, b| {
        b.density()
            .partial_cmp(&a.density())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.asn.cmp(&b.asn))
    });

    println!(
        "\n{:<10} {:>6} {:>11} {:>10}  {:<5} {}",
        "ASN", "hosts", "addresses", "density", "scan", "description"
    );
    let mut selected: Vec<&Candidate> = Vec::new();
    let mut budget = cli.max_addresses;
    for candidate in &candidates {
        let addresses = candidate.addresses();
        let fits = addresses <= budget;
        if fits {
            budget -= addresses;
            selected.push(candidate);
        }
        println!(
            "{:<10} {:>6} {:>11} {:>10}  {:<5} {}",
            format!("AS{}", candidate.asn),
            candidate.hosts,
            addresses,
            density_label(candidate),
            if fits { "yes" } else { "no" },
            candidate.description
        );
    }

    let collected: Vec<String> = selected
        .iter()
        .flat_map(|candidate| candidate.prefixes.iter().cloned())
        .collect();
    let prefixes = merge_prefixes(&collected);

    let addresses: u64 = prefixes.iter().filter_map(|p| prefix_addresses(p)).sum();
    info!(
        asns = selected.len(),
        prefixes = prefixes.len(),
        addresses,
        "Scan set selected"
    );

    let body = prefixes.join("\n") + "\n";
    match &cli.out {
        Some(path) => {
            std::fs::write(path, &body)
                .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", path.display()))?;
            info!(path = %path.display(), "Prefixes written");
        }
        None => print!("{body}"),
    }
    Ok(())
}

/// Read every host file and return the unique IPv4 addresses in them.
fn read_hosts(paths: &[String]) -> anyhow::Result<Vec<u32>> {
    let mut ips: Vec<u32> = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read hosts {path}: {e}"))?;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some(ip) = parse_host(line) {
                ips.push(ip);
            }
        }
    }
    ips.sort_unstable();
    ips.dedup();
    Ok(ips)
}

/// Accept `1.2.3.4` and `1.2.3.4:8000`; ignore anything that is not an IPv4.
fn parse_host(line: &str) -> Option<u32> {
    parse_ipv4(line.split(':').next()?)
}

fn default_table_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("aerial-registry").join("ip2asn-v4.tsv.gz")
}

async fn load_asn_table(
    client: &reqwest::Client,
    path: &Path,
    refresh: bool,
) -> anyhow::Result<Vec<AsnRange>> {
    if refresh || is_stale(path) {
        info!(url = ASN_TABLE_URL, "Downloading ASN table");
        let bytes = client
            .get(ASN_TABLE_URL)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, &bytes)
            .map_err(|e| anyhow::anyhow!("cannot cache ASN table {}: {e}", path.display()))?;
    }

    let raw = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("cannot read ASN table {}: {e}", path.display()))?;
    let mut decoder = flate2::read::GzDecoder::new(&raw[..]);
    let mut text = String::new();
    decoder
        .read_to_string(&mut text)
        .map_err(|e| anyhow::anyhow!("cannot decompress ASN table: {e}"))?;

    let mut ranges = parse_asn_table(&text);
    ranges.sort_by_key(|r| r.start);
    Ok(ranges)
}

fn is_stale(path: &Path) -> bool {
    match std::fs::metadata(path).and_then(|meta| meta.modified()) {
        Ok(modified) => modified
            .elapsed()
            .map(|age| age > TABLE_MAX_AGE)
            .unwrap_or(true),
        Err(_) => true,
    }
}

fn parse_asn_table(text: &str) -> Vec<AsnRange> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let start = parse_ipv4(fields.next()?)?;
            let end = parse_ipv4(fields.next()?)?;
            let asn: u32 = fields.next()?.trim().parse().ok()?;
            let _country = fields.next();
            let description = fields.next().unwrap_or("").trim().to_string();
            // ASN 0 is "Not routed"; skip it.
            (asn != 0).then_some(AsnRange {
                start,
                end,
                asn,
                description,
            })
        })
        .collect()
}

/// Parse a dotted-quad into its big-endian `u32` value.
fn parse_ipv4(text: &str) -> Option<u32> {
    text.trim().parse::<Ipv4Addr>().ok().map(u32::from)
}

/// Find the range containing `ip` (the table is sorted, non-overlapping).
fn lookup(table: &[AsnRange], ip: u32) -> Option<&AsnRange> {
    let index = table.partition_point(|range| range.start <= ip);
    let candidate = table.get(index.checked_sub(1)?)?;
    (ip <= candidate.end).then_some(candidate)
}

/// Count known hosts per ASN, dropping those below `min_hosts`, richest first.
fn rank_asns(table: &[AsnRange], ips: &[u32], min_hosts: usize) -> Vec<(u32, usize, String)> {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    let mut names: HashMap<u32, String> = HashMap::new();

    for &ip in ips {
        if let Some(range) = lookup(table, ip) {
            *counts.entry(range.asn).or_default() += 1;
            names
                .entry(range.asn)
                .or_insert_with(|| range.description.clone());
        }
    }

    let mut ranking: Vec<(u32, usize, String)> = counts
        .into_iter()
        .filter(|(_, count)| *count >= min_hosts)
        .map(|(asn, count)| {
            let name = names.remove(&asn).unwrap_or_default();
            (asn, count, name)
        })
        .collect();
    ranking.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranking
}

#[derive(Deserialize)]
struct RipeResponse {
    data: Option<RipeData>,
}

#[derive(Deserialize)]
struct RipeData {
    #[serde(default)]
    prefixes: Vec<RipePrefix>,
}

#[derive(Deserialize)]
struct RipePrefix {
    prefix: String,
}

/// Fetch the announced IPv4 prefixes of every ASN, concurrently.
/// Fetch the announced IPv4 prefixes of every ASN, concurrently, keeping them
/// grouped per ASN so each candidate's address count can be measured.
async fn fetch_prefixes(
    client: &reqwest::Client,
    asns: &[u32],
    concurrency: usize,
) -> Vec<(u32, Vec<String>)> {
    stream::iter(asns.iter().copied())
        .map(|asn| {
            let client = client.clone();
            async move {
                let url = format!("{RIPE_PREFIXES_URL}?resource=AS{asn}");
                match client.get(&url).send().await {
                    Ok(response) => match response.json::<RipeResponse>().await {
                        Ok(body) => {
                            let prefixes: Vec<String> = body
                                .data
                                .map(|d| d.prefixes)
                                .unwrap_or_default()
                                .into_iter()
                                .map(|p| p.prefix)
                                .filter(|p| p.contains('.'))
                                .collect();
                            info!(asn, prefixes = prefixes.len(), "Prefixes fetched");
                            (asn, prefixes)
                        }
                        Err(error) => {
                            warn!(asn, %error, "Unreadable RIPE Stat response");
                            (asn, Vec::new())
                        }
                    },
                    Err(error) => {
                        warn!(asn, %error, "RIPE Stat request failed");
                        (asn, Vec::new())
                    }
                }
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await
}

/// Collapse a prefix list into the minimal set of CIDR blocks: duplicates,
/// contained prefixes and adjacent ranges are merged, so **no address is probed
/// twice**. A union of per-ASN prefix lists routinely overlaps — the wide scan
/// target had 263 overlapping prefixes, ~915,000 wasted probes.
fn merge_prefixes(prefixes: &[String]) -> Vec<String> {
    let mut ranges: Vec<(u32, u32)> = prefixes.iter().filter_map(|p| prefix_range(p)).collect();
    ranges.sort_unstable();

    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
        .into_iter()
        .flat_map(|(start, end)| range_to_cidrs(start, end))
        .collect()
}

/// Parse `a.b.c.d/n` into its inclusive `(start, end)`.
fn prefix_range(prefix: &str) -> Option<(u32, u32)> {
    let (ip, bits) = prefix.split_once('/')?;
    let ip = parse_ipv4(ip)?;
    let bits: u32 = bits.trim().parse().ok()?;
    if bits > 32 {
        return None;
    }
    let mask = if bits == 0 {
        0
    } else {
        u32::MAX << (32 - bits)
    };
    let start = ip & mask;
    Some((start, start | !mask))
}

/// Split an inclusive range into the minimal set of CIDR blocks.
fn range_to_cidrs(start: u32, end: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = start;
    loop {
        let align_bits = if current == 0 {
            32
        } else {
            current.trailing_zeros()
        };
        let remaining = (end - current) as u64 + 1;
        let fit_bits = 63 - remaining.leading_zeros();
        let bits = align_bits.min(fit_bits);
        out.push(format!("{}/{}", Ipv4Addr::from(current), 32 - bits));
        if bits >= 32 {
            break;
        }
        current += 1u32 << bits;
        if current > end {
            break;
        }
    }
    out
}

/// Number of addresses in a `a.b.c.d/n` prefix.
fn prefix_addresses(prefix: &str) -> Option<u64> {
    let bits: u32 = prefix.split_once('/')?.1.parse().ok()?;
    (bits <= 32).then(|| 1u64 << (32 - bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
1.0.0.0\t1.0.0.255\t13335\tUS\tCLOUDFLARENET
1.0.1.0\t1.0.3.255\t0\tNone\tNot routed
10.0.0.0\t10.0.0.255\t64500\tGB\tEXAMPLE-AS
10.0.1.0\t10.0.1.255\t64500\tGB\tEXAMPLE-AS
10.0.2.0\t10.0.2.255\t64501\tDE\tOTHER-AS
";

    fn table() -> Vec<AsnRange> {
        let mut ranges = parse_asn_table(TABLE);
        ranges.sort_by_key(|r| r.start);
        ranges
    }

    #[test]
    fn skips_unrouted_ranges() {
        assert!(lookup(&table(), u32::from(Ipv4Addr::new(1, 0, 2, 5))).is_none());
    }

    #[test]
    fn looks_up_the_containing_asn() {
        let table = table();
        assert_eq!(
            lookup(&table, u32::from(Ipv4Addr::new(1, 0, 0, 7)))
                .unwrap()
                .asn,
            13335
        );
        assert_eq!(
            lookup(&table, u32::from(Ipv4Addr::new(10, 0, 1, 200)))
                .unwrap()
                .asn,
            64500
        );
        assert!(lookup(&table, u32::from(Ipv4Addr::new(9, 9, 9, 9))).is_none());
    }

    #[test]
    fn ranks_asns_by_host_count() {
        let ips = ["10.0.0.1", "10.0.0.2", "10.0.1.1", "10.0.2.1", "9.9.9.9"]
            .iter()
            .filter_map(|ip| parse_host(ip))
            .collect::<Vec<_>>();
        let ranking = rank_asns(&table(), &ips, 1);
        assert_eq!(ranking[0].0, 64500);
        assert_eq!(ranking[0].1, 3);
        assert_eq!(ranking[1].0, 64501);
        // `min_hosts` drops the single-host ASN.
        assert_eq!(rank_asns(&table(), &ips, 2).len(), 1);
    }

    #[test]
    fn parses_hosts_with_and_without_ports() {
        assert_eq!(parse_host("1.2.3.4"), Some(0x01020304));
        assert_eq!(parse_host("1.2.3.4:8000"), Some(0x01020304));
        assert_eq!(parse_host("radio.example.com"), None);
        assert_eq!(parse_host(""), None);
    }

    #[test]
    fn counts_prefix_addresses() {
        assert_eq!(prefix_addresses("10.0.0.0/8"), Some(1 << 24));
        assert_eq!(prefix_addresses("10.0.0.0/24"), Some(256));
        assert_eq!(prefix_addresses("10.0.0.0/32"), Some(1));
        assert_eq!(prefix_addresses("not-a-prefix"), None);
    }

    #[test]
    fn merges_duplicate_and_contained_prefixes() {
        // A /24 inside a /16 must not be scanned twice.
        let merged = merge_prefixes(&[
            "10.0.0.0/16".to_string(),
            "10.0.3.0/24".to_string(),
            "10.0.0.0/16".to_string(),
        ]);
        assert_eq!(merged, vec!["10.0.0.0/16"]);
        assert_eq!(prefix_addresses(&merged[0]), Some(65536));
    }

    #[test]
    fn merges_adjacent_ranges_into_the_smallest_blocks() {
        assert_eq!(
            merge_prefixes(&["10.0.0.0/25".to_string(), "10.0.0.128/25".to_string()]),
            vec!["10.0.0.0/24"]
        );
        // A partial overlap cannot be expressed as one block; it is summarised.
        let merged = merge_prefixes(&["10.0.0.0/24".to_string(), "10.0.0.128/25".to_string()]);
        assert_eq!(merged, vec!["10.0.0.0/24"]);
    }

    #[test]
    fn summarises_an_unaligned_range() {
        // 10.0.0.1 - 10.0.0.2 is not a single CIDR.
        assert_eq!(
            range_to_cidrs(
                u32::from(Ipv4Addr::new(10, 0, 0, 1)),
                u32::from(Ipv4Addr::new(10, 0, 0, 2))
            ),
            vec!["10.0.0.1/32", "10.0.0.2/32"]
        );
        assert_eq!(
            range_to_cidrs(
                u32::from(Ipv4Addr::new(10, 0, 0, 0)),
                u32::from(Ipv4Addr::new(10, 0, 0, 3))
            ),
            vec!["10.0.0.0/30"]
        );
    }

    #[test]
    fn merge_preserves_the_full_address_span() {
        let input = vec!["10.0.0.0/24".to_string(), "10.0.1.0/24".to_string()];
        let merged = merge_prefixes(&input);
        let before: u64 = input.iter().filter_map(|p| prefix_addresses(p)).sum();
        let after: u64 = merged.iter().filter_map(|p| prefix_addresses(p)).sum();
        assert_eq!(before, after);
        assert_eq!(after, 512);
    }
}

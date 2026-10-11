# Host discovery strategy

The registry's quality is bounded by the hosts we know about. Every discovery
path ends in the same place: a list of `ip:port` entries that
`aerial-registry icecast-scan file <path>` enumerates via `/status-json.xsl`.
This document describes how we generate that list.

## Why not scan the internet

Blind IPv4 scanning is the wrong shape:

| | |
| --- | --- |
| addresses | ~4.3 B |
| ports worth probing | ~9 |
| probes | ~39 B |
| throughput at 1024 concurrent / 3 s timeout | ~340 probes/s |
| time to finish | **~3.6 years** |

A SYN scanner (masscan/ZMap) at 1 M pps would finish in ~11 hours, but needs a
VPS with a good NIC, is indistinguishable from an attack, and breaches most
hosting providers' terms of service. Shodan, Censys and Netlas exist because
this is hard.

**So: only ever scan address space we have a reason to believe hosts radio.**

## The loop

```text
targets --hosts icecast-hosts.txt --max-addresses 2000000 --out prefixes.txt
portscan --targets prefixes.txt --append --out icecast-hosts.txt
aerial-registry icecast-scan file icecast-hosts.txt
```

Each round widens the seed file, which sharpens the next round's density
ranking, which targets the next scan. The seed file is the only state.

## Phase 1 — Target generation

Ranked by expected hit rate per probe.

### 1. ASN expansion from hosts we already know (highest yield)

Icecast servers cluster in a small number of hosting providers. Rather than
sampling the whole internet, find the providers that already host radio and
scan those. `src/bin/targets.rs` implements this:

1. Take every known netloc — Radio-Browser-derived enumeration, Xiph YP,
   Netlas, previous scans.
2. Resolve each IP to its ASN with a local `iptoasn.com` table (cached in
   `~/.cache/aerial-registry/`), not one API call per IP.
3. Fetch each candidate ASN's announced prefixes from RIPE Stat.
4. **Rank by density — known hosts per announced address — not by host
   count**, and select ASNs until an address budget is reached.

Ranking by raw host count is a trap, because the biggest clouds dominate the
count and are barely better than the open internet:

| ASN | hosts | addresses | density |
| --- | --- | --- | --- |
| AS141107 Linkgo (ID) | 2 | 2,816 | **1 / 1,408** |
| AS29889 FSNET-1 | 3 | 9,984 | 1 / 3,328 |
| AS40021 Contabo | 10 | 145,408 | 1 / 14,541 |
| AS25182 NPO (NL public broadcaster) | 3 | 65,536 | 1 / 21,845 |
| AS14061 DigitalOcean | 30 | 3,141,888 | 1 / 104,730 |
| AS16509 Amazon-02 | 2 | 219,091,200 | 1 / 109,545,600 |

For reference the open internet averages roughly 1 Icecast host per 161,000
addresses (26,684 hosts over ~4.3 B addresses). DigitalOcean is therefore only
~1.5× better than scanning at random, and Amazon is ~680× *worse*. A budget
spent on host count would have gone almost entirely to those two.

Density is estimated from hosts we already know, so it is a floor on the true
density, not the true density — the point of scanning is to find the hosts our
sources do not list. Measure the result rather than trusting the estimate.

### 2. Reverse DNS (PTR) name filtering

Hosts whose PTR contains `radio`, `fm`, `stream`, `icecast`, `shoutcast`,
`azura` or `listen` are strong candidates. Probe them directly — no port scan
needed, since the name tells us the likely port.

### 3. Certificate Transparency

Query CT logs (crt.sh) for `%.radio.%`, `%stream%`, `%fm.%` style names,
resolve the results, and probe. Public, free, and gives hostnames rather than
addresses.

### 4. Public stream datasets

Datasets that already contain stream URLs give host and port directly:

- `junguler/m3u-radio-music-playlists` (GitHub Pages, large)
- `fmstream.org`
- `mikepierce/internet-radio-streams`
- TuneIn's OPML API (`opml.radiotime.com`)

These are aggregators, so treat the results like Radio-Browser: candidates to
enumerate and dedupe, not authorities.

### 5. Gated bulk sources (not currently available)

Rapid7 Project Sonar and Censys bulk data would be ideal — `http_get_8000.json.gz`
is literally "every host on port 8000 with an HTTP response", and Sonar's HTTP
study includes the `Server:` header. Both are now behind research/commercial
vetting. Rapid7 closed public access on 10 February 2022; Censys grants free
top-tier access to verified researchers. Worth applying for, not worth
depending on.

## Phase 2 — Scan

`portscan` is a standalone connect scanner. Its only output is `ip:port`
lines, so it composes with the enumerator without either knowing about the
other.

```
portscan --targets prefixes.txt --append --out icecast-hosts.txt
portscan 203.0.113.0/24 --ports 8000-8010
```

Default ports are a curated Icecast/Shoutcast subset:
`8000,8001,8008,8010,8080,8081,8443,8888,9000`. This is a **subset** of the 62
ports Sonar's HTTP study covered, so Icecast listening on an unlisted port is
invisible. Probe cost is linear in ports, so widening `--ports` is a direct
trade of coverage against time.

Politeness: keep `--concurrency` modest (1024 is a reasonable ceiling), keep
the timeout short, and scan from a host whose provider permits it.

### Throughput

Two measurements, both with a 2 s timeout:

| run | concurrency | probes | wall time | rate | open ports |
| --- | --- | --- | --- | --- | --- |
| pilot (2 densest ASNs) | 512 | 115,200 | 6 m 56 s | ~277/s | 225 (0.20 %) |
| test (79 prefixes) | 1024 | 601,344 | 16 m 17 s | ~616/s | 5,118 (0.85 %) |

The rate is dominated by the connect timeout, not by the open ports: most
probes go to addresses that neither accept nor refuse, so each one waits the
full timeout. Throughput therefore scales roughly with `concurrency / timeout`
rather than with bandwidth, and `--concurrency` is the main lever. Raising it
makes the scan look more like an attack, so trade carefully.

### Yields, and why a small pilot misleads

The open-port rate varies by more than 4× depending on which ASNs are sampled:
the pilot's two small ISPs gave 0.20 %, while a slice spanning VPS providers
gave 0.85 %. **Do not extrapolate from a pilot of two ASNs** — it measures the
sampling, not the strategy.

Of the open ports, only **0.9 %** are Icecast, so port-based discovery is very
noisy: 5,118 open ports yielded 44 servers. That is acceptable because
enumeration is cheap — 5,118 hosts in 80 s — and because the scanner stays
protocol-agnostic rather than growing its own Icecast check.

The encouraging part: **43 of those 44 (98 %) were not already in the seed
file**, so the scan discovers rather than rediscovers. Expect that ratio to
fall as the seed grows.

## Phase 3 — Enumerate

```
aerial-registry icecast-scan file icecast-hosts.txt --responders responders.tsv
```

This asks each host for `/status-json.xsl` and reports how many responded and
how many mounts they expose. It is the step that distinguishes a real Icecast
server from anything else that happens to listen on port 8000.

`--responders` writes every mount of every responding host as
`host<TAB>mount name`, one line per mount. **Always measure with this file.**
Re-implementing the enumeration in a script duplicates the logic, diverges from
it over time, and is how a scan gets reported as yielding nothing when it
actually yielded something.

## Phase 4 — Integration

Scanning is expensive and occasional; enumeration is cheap. So:

- Run the scan **periodically** (weekly/monthly), not nightly, and append to
  the seed file.
- Have the nightly pipeline re-enumerate the cached seed file as a provider.
  Dead hosts fall out through the normal liveness path; new hosts stay in the
  seed.

This keeps the nightly job's cost flat while the host list grows.

## Phase 5 — Measuring

Each phase should report a yield so the strategy can be corrected:

| metric | meaning |
| --- | --- |
| hosts scanned | probes spent |
| ports open | TCP responders |
| Icecast responders | enumeration hit rate |
| new stations after dedup | actual value added |

If ASN targeting does not beat the ~58–60 % enumeration hit rate we already see
from Netlas hosts, the target generation is wrong, not the scanner.

## Risks

- **Terms of service.** Scanning is legal in many jurisdictions but commonly
  forbidden by hosting providers. Prefer prefixes that already host radio, and
  keep rates low.
- **Abuse reports.** A fast scan looks like an attack. Concurrency caps and a
  recognisable reverse DNS for the scanning host help.
- **Only GET.** We fetch the public status endpoint and nothing else. No
  authentication attempts, no exploitation.

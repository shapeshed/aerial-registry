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

## Phase 1 — Target generation

Ranked by expected hit rate per probe.

### 1. ASN expansion from hosts we already know (highest yield)

Icecast servers cluster in a small number of hosting providers. Rather than
sampling the whole internet, find the providers that already host radio and
scan those.

1. Take every known netloc — Radio-Browser-derived enumeration, Xiph YP,
   Netlas, previous scans.
2. Resolve each IP to its ASN. Use a local IP→ASN table (e.g. the
   `iptoasn.com` `ip2asn-v4.tsv.gz` dump) rather than one API call per IP.
3. Rank ASNs by how many known Icecast hosts they contain.
4. Pull each ASN's announced prefixes (RIPE Stat `announced-prefixes`, or a
   BGP dump).
5. Scan the top N ASNs' prefixes on the Icecast ports.

A few hundred radio-heavy ASNs is a bounded, scannable candidate set — and a
far better use of probes than an equal number of random `/8` blocks.

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

Default ports are the Icecast/Shoutcast set Sonar used:
`8000,8001,8008,8010,8080,8081,8443,8888,9000`.

Politeness: keep `--concurrency` modest (1024 is a reasonable ceiling), keep
the timeout short, and scan from a host whose provider permits it.

## Phase 3 — Enumerate

```
aerial-registry icecast-scan file icecast-hosts.txt
```

This asks each host for `/status-json.xsl` and reports how many responded and
how many mounts they expose. It is the step that distinguishes a real Icecast
server from anything else that happens to listen on port 8000.

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

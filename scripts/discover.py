#!/usr/bin/env python3
"""
Radio station discovery agent.

Queries Radio Browser for each target country, applies mechanical quality
filters, fetches og:image logos, then uses Claude to assess quality and clean
station names. Approved entries are appended directly to stations.toml.

Usage:
    pip install anthropic requests
    python scripts/discover.py --countries GB DE FR
    python scripts/discover.py --countries GB --min-votes 1000
"""

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path
from urllib.parse import urlparse

from concurrent.futures import ThreadPoolExecutor, as_completed

import anthropic
import requests

RADIO_BROWSER_API = "https://de1.api.radio-browser.info/json/stations/search"
REGISTRY_URL = "https://aerial.shapeshed.com/registry.json.gz"
STATIONS_TOML = Path(__file__).parent.parent / "stations.toml"

# Stream domains already covered by broadcaster providers — skip these.
KNOWN_PROVIDER_DOMAINS = {
    "musicradio.com",
    "thisisdax.com",
    "bbcmedia.co.uk",
    "bbci.co.uk",
    "akamaized.net",
    "talksport.com",
    "talkradio.co.uk",
    "virginradio.co.uk",
    "wireless.radio",
    "ardaudiothek.de",
    "radiofrance.fr",
    "rtve.es",
}

COUNTRY_NAMES = {
    "GB": "United Kingdom",
    "DE": "Germany",
    "FR": "France",
    "ES": "Spain",
    "IT": "Italy",
    "NL": "Netherlands",
    "AT": "Austria",
    "CH": "Switzerland",
    "SE": "Sweden",
    "NO": "Norway",
    "DK": "Denmark",
    "FI": "Finland",
    "PL": "Poland",
    "PT": "Portugal",
    "IE": "Ireland",
}


def normalise_url(url: str) -> str:
    return (
        url.lower()
        .removeprefix("https://")
        .removeprefix("http://")
        .rstrip("/")
        .split("?")[0]
    )


def is_raw_ip(url: str) -> bool:
    try:
        host = urlparse(url).netloc.split(":")[0]
        return host.replace(".", "").isdigit()
    except Exception:
        return False


def load_existing() -> tuple[set[str], set[str]]:
    """
    Return (normalised_stream_urls, lower_names) from two sources:
    1. stations.toml — hand-curated entries in this repo
    2. Live registry at REGISTRY_URL — the compiled output of all providers

    The registry is canonical. If a station is already in it under any name
    or URL, we skip it — we don't want to add Radio Browser's alternative
    stream URL for a station the broadcaster providers already cover.
    """
    urls: set[str] = set()
    names: set[str] = set()

    # Load stations.toml
    if STATIONS_TOML.exists():
        with open(STATIONS_TOML, "rb") as f:
            data = tomllib.load(f)
        for s in data.get("stations", []):
            urls.add(normalise_url(s["stream_url"]))
            names.add(s["name"].lower())

    # Load live registry — requests auto-decompresses Content-Encoding: gzip
    try:
        resp = requests.get(REGISTRY_URL, timeout=15)
        resp.raise_for_status()
        registry = resp.json()
        stations = registry if isinstance(registry, list) else registry.get("stations", [])
        for s in stations:
            if s.get("stream_url"):
                urls.add(normalise_url(s["stream_url"]))
            if s.get("name"):
                names.add(s["name"].lower())
        print(f"  Loaded {len(stations)} stations from live registry", file=sys.stderr)
    except Exception as e:
        print(f"  Warning: could not load live registry ({e}) — deduping against stations.toml only", file=sys.stderr)

    return urls, names


def fetch_candidates(country_code: str, min_votes: int) -> list[dict]:
    resp = requests.get(
        RADIO_BROWSER_API,
        params={
            "countrycode": country_code,
            "hidebroken": "true",
            "order": "votes",
            "reverse": "true",
            "limit": "10000",
        },
        timeout=30,
    )
    resp.raise_for_status()

    out = []
    for s in resp.json():
        url = s.get("url", "")
        if not url:
            continue
        if s.get("votes", 0) < min_votes:
            continue
        if is_raw_ip(url):
            continue
        if s.get("bitrate", 0) < 64:
            continue
        domain = urlparse(url).netloc
        if any(kd in domain for kd in KNOWN_PROVIDER_DOMAINS):
            continue
        out.append(s)

    return out


def fetch_og_image(url: str, timeout: int = 5) -> str | None:
    try:
        parsed = urlparse(url)
        homepage = f"{parsed.scheme}://{parsed.netloc}"
        resp = requests.get(homepage, timeout=timeout, allow_redirects=True)
        html = resp.text
        m = re.search(
            r'<meta[^>]+property=["\']og:image["\'][^>]+content=["\'](https?://[^"\']+)["\']',
            html,
        )
        if m:
            return m.group(1)
        m = re.search(
            r'<meta[^>]+content=["\'](https?://[^"\']+)["\'][^>]+property=["\']og:image["\']',
            html,
        )
        if m:
            return m.group(1)
    except Exception:
        pass
    return None


def assess_with_claude(client: anthropic.Anthropic, candidates: list[dict]) -> list[dict]:
    if not candidates:
        return []

    station_list = "\n".join(
        f"{i+1}. name={s['name']!r} url={s['url']!r} votes={s['votes']} bitrate={s['bitrate']}k tags={s.get('tags','')!r}"
        for i, s in enumerate(candidates)
    )

    prompt = f"""You are reviewing candidate radio stations for a curated internet radio registry used in a polished consumer app.

For each station, decide:
1. Should it be included? (include: true/false)
2. A clean, properly-capitalised display name — fix ALL-CAPS, strip codec/quality suffixes like [MP3] (128k), strip leading/trailing symbols
3. Up to 5 tags from: pop, rock, classical, jazz, electronic, dance, trance, house, hip-hop, indie, alternative, folk, country, news, talk, sport, world, ambient, metal, punk, reggae, soul, r&b, oldies, comedy, culture, public radio
4. One-line reason

Exclude if:
- Name is spammy/junk (e.g. "# TOP 100 DJ CHARTS", "__TRANCE__ by rautemusik", "BEST SMOOTH JAZZ - UK")
- Pure internet aggregator with no real station identity
- Geographically misleading for its listed country
- Duplicate of a well-known branded station (BBC, Global Radio, etc.)

Include if:
- Real named station with an identity and genuine audience
- Reasonable vote count relative to peers

Stations:
{station_list}

Respond with a JSON array only (no markdown), one object per station in order:
[{{"index": 1, "include": true, "cleaned_name": "...", "tags": ["tag1"], "reason": "..."}}, ...]"""

    message = client.messages.create(
        model="claude-haiku-4-5-20251001",
        max_tokens=4096,
        messages=[{"role": "user", "content": prompt}],
    )

    return json.loads(message.content[0].text.strip())


def format_toml_entry(
    name: str,
    country_code: str,
    stream_url: str,
    logo_url: str | None,
    tags: list[str],
) -> str:
    lines = [
        "[[stations]]",
        f"name = {json.dumps(name)}",
        f"country_code = {json.dumps(country_code)}",
        f"stream_url = {json.dumps(stream_url)}",
    ]
    if logo_url:
        lines.append(f"logo_url = {json.dumps(logo_url)}")
    if tags:
        tag_str = ", ".join(f'"{t}"' for t in tags)
        lines.append(f"tags = [{tag_str}]")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--countries", nargs="+", default=["GB"], metavar="CC")
    parser.add_argument("--min-votes", type=int, default=500)
    parser.add_argument("--batch-size", type=int, default=20)
    args = parser.parse_args()

    client = anthropic.Anthropic()

    print("Loading existing stations...", file=sys.stderr)
    existing_urls, existing_names = load_existing()
    print(f"  {len(existing_urls)} known stream URLs (stations.toml + live registry)", file=sys.stderr)

    approved: list[str] = []

    for country_code in args.countries:
        country_name = COUNTRY_NAMES.get(country_code, country_code)
        print(f"\n=== {country_name} ({country_code}) ===", file=sys.stderr)

        print("  Fetching Radio Browser...", file=sys.stderr)
        candidates = fetch_candidates(country_code, args.min_votes)
        print(f"  {len(candidates)} candidates after quality filter (votes >={args.min_votes}, >=64k, not raw IP, not known provider)", file=sys.stderr)

        fresh = [
            s for s in candidates
            if normalise_url(s["url"]) not in existing_urls
            and s["name"].lower() not in existing_names
        ]
        skipped = len(candidates) - len(fresh)
        print(f"  {skipped} skipped (already in registry), {len(fresh)} new to assess", file=sys.stderr)

        if not fresh:
            print("  Nothing new — skipping.", file=sys.stderr)
            continue

        print(f"  Fetching logos for {len(fresh)} stations (10 concurrent, 3s timeout)...", file=sys.stderr)

        def _fetch_logo(s: dict) -> tuple[str, str | None]:
            logo = s.get("favicon") or fetch_og_image(s["url"], timeout=3)
            return s["url"], logo

        with ThreadPoolExecutor(max_workers=10) as pool:
            futures = {pool.submit(_fetch_logo, s): s for s in fresh}
            logo_map: dict[str, str | None] = {}
            for future in as_completed(futures):
                url, logo = future.result()
                logo_map[url] = logo

        with_logo = sum(1 for v in logo_map.values() if v)
        print(f"  Logos found: {with_logo}/{len(fresh)}", file=sys.stderr)

        for s in fresh:
            s["_logo"] = logo_map.get(s["url"])

        n_batches = (len(fresh) + args.batch_size - 1) // args.batch_size
        print(f"  Sending {len(fresh)} stations to Claude in {n_batches} batch(es)...", file=sys.stderr)

        for i in range(0, len(fresh), args.batch_size):
            batch = fresh[i:i + args.batch_size]
            batch_num = i // args.batch_size + 1
            print(f"  Batch {batch_num}/{n_batches} ({len(batch)} stations)...", file=sys.stderr)

            try:
                assessments = assess_with_claude(client, batch)
            except Exception as e:
                print(f"  Claude error on batch {batch_num}: {e}", file=sys.stderr)
                continue

            batch_added = 0
            batch_skipped = 0
            for assessment in assessments:
                idx = assessment["index"] - 1
                if idx >= len(batch):
                    continue
                station = batch[idx]
                if not assessment.get("include"):
                    print(f"    SKIP  {station['name']!r}: {assessment.get('reason', '')}", file=sys.stderr)
                    batch_skipped += 1
                    continue

                entry = format_toml_entry(
                    name=assessment["cleaned_name"],
                    country_code=country_code,
                    stream_url=station["url"],
                    logo_url=station.get("_logo"),
                    tags=assessment.get("tags", []),
                )
                approved.append(entry)
                existing_urls.add(normalise_url(station["url"]))
                existing_names.add(assessment["cleaned_name"].lower())
                print(f"    ADD   {assessment['cleaned_name']!r}: {assessment.get('reason', '')}", file=sys.stderr)
                batch_added += 1

            print(f"  Batch {batch_num} done: {batch_added} added, {batch_skipped} skipped", file=sys.stderr)

    print(f"\n{'='*40}", file=sys.stderr)
    if not approved:
        print("No new stations approved.", file=sys.stderr)
        return

    print(f"Total approved: {len(approved)} stations", file=sys.stderr)

    block = "\n\n".join(approved) + "\n"
    with open(STATIONS_TOML, "a") as f:
        f.write("\n" + block)

    print(f"Appended to stations.toml.", file=sys.stderr)


if __name__ == "__main__":
    main()

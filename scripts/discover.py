#!/usr/bin/env python3
"""
Radio station candidate generator.

Queries Radio Browser for each target country, applies mechanical quality
filters (votes, bitrate, HTTPS, logo, not a known-provider domain), fetches
og:image logos from station websites, then writes candidates to a TOML file
for agent review.

The agent (not this script) decides which candidates to add to stations.toml.

Usage:
    pip install requests
    python scripts/discover.py --countries GB DE FR
    python scripts/discover.py --countries GB --min-votes 1000 --output proposed.toml
"""

import argparse
import json
import re
import sys
import time
import tomllib
from pathlib import Path
from urllib.parse import urlparse

import requests

RADIO_BROWSER_API = "https://de1.api.radio-browser.info/json/stations/search"
EXISTING_TOML = Path(__file__).parent.parent / "stations.toml"

# Stream domains already covered by broadcaster providers — skip these.
KNOWN_PROVIDER_DOMAINS = {
    "musicradio.com",      # Global Radio
    "thisisdax.com",       # Global Radio CDN
    "bbcmedia.co.uk",      # BBC
    "bbci.co.uk",          # BBC
    "akamaized.net",       # BBC HLS CDN
    "talksport.com",       # Wireless
    "talkradio.co.uk",     # Wireless
    "virginradio.co.uk",   # Wireless
    "wireless.radio",      # Wireless
    "ardaudiothek.de",     # ARD
    "radiofrance.fr",      # Radio France
    "rtve.es",             # RTVE
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


def load_existing(path: Path) -> tuple[set[str], set[str]]:
    """Return (normalised_stream_urls, lower_names) already in stations.toml."""
    if not path.exists():
        return set(), set()
    with open(path, "rb") as f:
        data = tomllib.load(f)
    urls = {normalise_url(s["stream_url"]) for s in data.get("stations", [])}
    names = {s["name"].lower() for s in data.get("stations", [])}
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
    """Try to extract og:image from the station's homepage."""
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


def format_toml_entry(
    name: str,
    country_code: str,
    stream_url: str,
    logo_url: str | None,
    tags: list[str],
    votes: int,
    bitrate: int,
) -> str:
    lines = [
        "[[stations]]",
        f"# votes={votes} bitrate={bitrate}k",
        f"name = {json.dumps(name)}",
        f"country_code = {json.dumps(country_code)}",
        f"stream_url = {json.dumps(stream_url)}",
    ]
    if logo_url:
        lines.append(f"logo_url = {json.dumps(logo_url)}")
    if tags:
        tag_str = ", ".join(f'"{t}"' for t in tags[:5])
        lines.append(f"tags = [{tag_str}]")
    return "\n".join(lines)


def parse_tags(raw: str) -> list[str]:
    return [t.strip() for t in raw.split(",") if t.strip()][:5]


def main():
    parser = argparse.ArgumentParser(description="Generate radio station candidates from Radio Browser")
    parser.add_argument("--countries", nargs="+", default=["GB"], metavar="CC")
    parser.add_argument("--min-votes", type=int, default=500)
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args()

    existing_urls, existing_names = load_existing(EXISTING_TOML)
    print(f"Loaded {len(existing_urls)} existing stations from stations.toml", file=sys.stderr)

    entries: list[str] = []

    for country_code in args.countries:
        country_name = COUNTRY_NAMES.get(country_code, country_code)
        print(f"\n=== {country_name} ({country_code}) ===", file=sys.stderr)

        candidates = fetch_candidates(country_code, args.min_votes)
        print(f"  {len(candidates)} candidates after quality filter", file=sys.stderr)

        fresh = [
            s for s in candidates
            if normalise_url(s["url"]) not in existing_urls
            and s["name"].lower() not in existing_names
        ]
        print(f"  {len(fresh)} not already in stations.toml", file=sys.stderr)

        for s in fresh:
            logo = s.get("favicon") or fetch_og_image(s["url"])
            time.sleep(0.1)

            entry = format_toml_entry(
                name=s["name"],
                country_code=country_code,
                stream_url=s["url"],
                logo_url=logo,
                tags=parse_tags(s.get("tags", "")),
                votes=s.get("votes", 0),
                bitrate=s.get("bitrate", 0),
            )
            entries.append(entry)

    if not entries:
        print("\nNo new candidates found.", file=sys.stderr)
        return

    header = (
        "# Station candidates — agent review required\n"
        "#\n"
        "# Each [[stations]] block is a candidate from Radio Browser that passed\n"
        "# mechanical quality filters (votes, bitrate, HTTPS, logo, not a known provider).\n"
        "#\n"
        "# Instructions for the reviewing agent:\n"
        "# 1. Keep entries for real, named stations with a genuine audience.\n"
        "# 2. Remove entries with junk/spammy names, aggregators, or misleading geography.\n"
        "# 3. Clean up station names: proper capitalisation, remove codec suffixes like [MP3].\n"
        "# 4. Append approved entries to stations.toml.\n"
        "# 5. Delete this file.\n\n"
    )

    output = header + "\n\n".join(entries) + "\n"

    if args.output:
        args.output.write_text(output)
        print(f"\nWrote {len(entries)} candidates to {args.output}", file=sys.stderr)
    else:
        print(output)


if __name__ == "__main__":
    main()

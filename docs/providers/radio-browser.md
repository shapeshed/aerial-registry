# Radio Browser Provider

Radio Browser (<https://www.radio-browser.info>) is a community-maintained open
catalogue of internet radio stations. No authentication is required.

Radio Browser serves two roles in the registry pipeline:

1. **Bulk provider** (`src/providers/radio_browser.rs`) — the untrusted
   long-tail source for everything the ~40 broadcaster-direct providers and
   `curated` don't cover. See "Bulk Discovery" below.
2. **Tag enrichment source** (`pipeline::enrich.rs`) — a name-based lookup
   that adds tags to stations discovered from *other* providers. Skipped
   entirely for stations this provider itself discovered — they already
   carry their own tags from the bulk fetch.

## Server Discovery

Radio Browser runs multiple independent servers, and any one of them can be
the one currently returning 502s. Discover the live server list before
making any other request:

```
GET https://all.api.radio-browser.info/json/servers
```

Returns a JSON array of server objects; each has a `name` field containing
the hostname. `src/radio_browser_client.rs` rotates through them on each
retry (with exponential backoff — 500ms, 1s, 2s) rather than hammering one
host. If server discovery itself fails, the bulk provider returns no
stations for that run — the guard (see "Interactions" below) is what stops
that from actually publishing as a near-empty catalog.

## Bulk Discovery

Paginate the full catalog:

```
GET https://{server}/json/stations/search?hidebroken=true&order=name&offset={n}&limit=5000
```

`order=name` keeps pagination stable across pages. Continue until a page
returns fewer than the requested limit (end of data) or a safety cap on page
count is hit. Each page fetch independently retries/rotates mirrors on
failure; if one page's retries are exhausted, pagination stops there and
whatever has been collected so far is returned rather than looping on a dead
upstream.

`hidebroken=true` omits stations Radio Browser has already detected as dead
on its own infrastructure; `pipeline::liveness`'s three-strike hysteresis
catches anything that slips through or goes dead later.

### Response fields used

| Field          | Notes                                                    |
| -------------- | --------------------------------------------------------- |
| `stationuuid`  | Stable per-station id — becomes `provider_id`             |
| `name`         | Contains codec/bitrate noise; see "Name Cleaning"          |
| `url_resolved` | Resolved stream URL — required, becomes `stream_url`       |
| `favicon`      | Logo URL, only kept if it starts with `http`                |
| `country` / `countrycode` | Passed through as-is                          |
| `tags`         | Comma-separated; lowercased and deduplicated                |
| `votes` / `clickcount` | Used only to score within-provider duplicates (see below), not stored |

## Name Cleaning

Radio Browser station names frequently contain appended codec and bitrate
information. Strip trailing noise before storing:

- Bracketed suffixes: `[MP3]`, `[128k]`
- Parenthesised codec/quality: `(AAC)`, `(128k)`, `(HQ)`, `(Medium Bitrate)`
- Pipe-separated technical suffixes: `| MP3 128k`
- Dash-prefixed format: `- MP3`, `- AAC HD 256k`
- Bare trailing format + bitrate: `AAC 256k`, `MP3 128k`
- Bare trailing quality words: `HQ`, `HD`

## Deduplication Within Radio Browser Results

Group records by normalised stream URL (lowercase, strip scheme, strip trailing
slash, drop query string). Within each group keep the record with the highest
score:

```
score = votes * 2 + clickcount
```

If the winning record has no favicon, take the favicon from any other record in
the group that has one.

Two fields are always fixed rather than sourced from the API: `provider` is
always `"radio-browser"`, and `trusted` is always `false` — this is the one
provider explicitly not treated as authoritative (see "Interactions" below).
`description` is never set; Radio Browser doesn't supply one.

## Interactions with the rest of the pipeline

- **Dedup (`pipeline::dedup`).** Once a trusted provider has an entry for the
  same `(name, country_code)`, any Radio Browser entry there is dropped —
  even if the stream URL doesn't match, since a broadcaster's own CDN mirror
  rarely matches the URL Radio Browser happened to index. This is separate
  from the within-provider dedup above, which only merges Radio Browser's
  own duplicate submissions of one stream.
- **Guard (`pipeline::guard`).** If the bulk fetch fails outright, the guard
  compares against the previous registry and carries the prior run's Radio
  Browser entries forward instead of publishing a near-empty catalog.
- **Overlay (`pipeline::overlay`).** Known-bad individual entries (a
  dead/superseded stream URL, a broken logo, a wrong name) are corrected or
  excluded by hand via `overlays/radio-browser/<COUNTRY>.toml` — see that
  directory's README.

## API Behaviour Notes

- **No authentication required.** A descriptive `User-Agent` is still sent
  on every request (`src/http.rs`), since Radio Browser asks clients to
  identify themselves.
- **Expect occasional 502s.** This is the most failure-prone provider in the
  pipeline. Mirror rotation + retry-with-backoff and the guard's
  carry-forward protection both exist specifically because of this.
- **`stationuuid` is the identity to key on**, not the station name — names
  collide constantly across unrelated stations in different countries.
- **`hidebroken=true`** omits stations Radio Browser has already detected as
  dead on its own infrastructure; `pipeline::liveness`'s three-strike
  hysteresis catches anything that slips through or goes dead later.

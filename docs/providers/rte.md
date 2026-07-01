# RTÉ Provider

RTÉ (Raidió Teilifís Éireann) is the Irish public broadcaster. Its live radio
lineup is available through a small public, unauthenticated JSON endpoint,
with streams served via StreamTheWorld behind a stable manifest redirect.

## Station Discovery

### Live stations endpoint

```
GET https://www.rte.ie/radio/live_stations/json
```

Returns `{"stations": [...]}` — 5 stations at time of writing.

| Field           | Notes                                                    |
| ---------------- | ---------------------------------------------------------- |
| `slug`            | Stable identifier — used as `provider_id`, and as input to the manifest URL (see below) |
| `name`            | Station display name, e.g. `RTÉ Radio 1`                   |
| `logoSvgUrl`      | Direct SVG logo URL                                        |
| `description`     | Always empty in practice; drop if so                       |

### Stream URL

```
https://www.rte.ie/manifests/{manifestSlug}.m3u8
```

**The manifest slug does not always match the API slug** — the live_stations
API returns `lyricfm`, but `lyricfm.m3u8` 404s; the actual manifest is at
`lyric.m3u8`. All other slugs (`radio1`, `2fm`, `rnag`, `gold`) match as-is.

This URL redirects (302) to a StreamTheWorld HLS URL containing a per-request
session ID — store the stable `rte.ie/manifests/...` URL, not the resolved
target, since the session ID is not guaranteed to stay valid.

## Data Points

| Field          | Source            | Notes                                             |
| -------------- | ------------------- | -------------------------------------------------- |
| `name`         | `name`               |                                                    |
| `stream_url`   | Constructed manifest URL | See slug mapping above                        |
| `logo_url`     | `logoSvgUrl`         |                                                    |
| `country`      | Hardcoded            | Always `Ireland` / `IE`                            |
| `description`  | `description`        | Always empty in practice                           |
| `provider_id`  | `slug`               |                                                    |
| `trusted`      | Hardcoded            | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **Only 5 stations** — RTÉ Radio 1, 2FM, Lyric FM, Raidió na Gaeltachta, and
  Gold. No regional variants.
- **`lyricfm` → `lyric` slug mismatch** is the only real gotcha; every other
  slug is used as-is for both the API and the manifest path.

# RTBF Provider

RTBF (Radio-Télévision Belge Francophone) is the public broadcaster for
Belgium's French-speaking (Wallonia/Brussels) community. Its full radio and
webradio lineup is available through a single public, unauthenticated REST
endpoint — no GraphQL, no auth, no per-station resolution needed.

## Station Discovery

### Channels endpoint

```
GET https://bff-service.rtbf.be/oaos/v1.6/channels?_sort%5Bid%5D=asc&types=radio%2Cwebradio&_limit=500&platform=WEB&excludeIds=132%2C140
```

`types=radio,webradio` already scopes the response to audio channels (no TV);
`_limit=500` returns everything in one page (32 channels at time of writing —
`meta.page.last` confirms no further pages exist). Response shape:
`{"data": [...]}`.

| Field                | Notes                                                            |
| --------------------- | -------------------------------------------------------------------- |
| `key`                  | Stable slug — used as `provider_id`                                  |
| `label`                | Station display name                                                 |
| `tagline`              | Always `null` in practice; drop if empty                             |
| `streamUrl.aac`        | Preferred stream variant                                              |
| `streamUrl.mp3`        | Fallback if `aac` is absent (a few webradios only have MP3)           |
| `logoFlat.light.png`   | Square icon — only present on 5 flagship stations                    |
| `logo.light.png`       | Wide wordmark — present on all 32 stations                           |

### Logo

Only the 5 flagship stations (Classic 21, La Première, Musiq3, Vivacité,
Tipik) have a `logoFlat` (a genuinely square, ~50×50 icon). Every other
station — the 7 regional Vivacité variants and the 20 webradio genre/decade
channels — only has `logo`, a wide wordmark (~512×214). This provider prefers
`logoFlat` and falls back to `logo` so every station gets some logo rather
than none, at the cost of some being non-square.

## Data Points

| Field          | Source                        | Notes                                             |
| -------------- | ------------------------------- | -------------------------------------------------- |
| `name`         | `label`                          |                                                    |
| `stream_url`   | `streamUrl.aac`                  | Falls back to `streamUrl.mp3`                      |
| `logo_url`     | `logoFlat.light.png`             | Falls back to `logo.light.png` (wordmark, not square) |
| `country`      | Hardcoded                       | Always `Belgium` / `BE`                            |
| `description`  | `tagline`                        | Always empty in practice                           |
| `provider_id`  | `key`                             |                                                    |
| `trusted`      | Hardcoded                       | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **Single request** returns the full lineup — 16 main "radio" stations
  (including 7 regional Vivacité variants) and 16 "webradio" thematic
  channels (mostly Classic 21/Musiq3 sub-genres).
- **Most stations have no square logo.** Only check `logoFlat`, don't assume
  it's always present.

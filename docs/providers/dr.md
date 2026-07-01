# DR Provider

DR (Danmarks Radio) is the Danish public broadcaster. Its full radio channel
lineup — national and regional — is available through a single public,
unauthenticated JSON endpoint.

## Station Discovery

### Channels endpoint

```
GET https://api.dr.dk/radio/v5/channels
```

Returns a plain JSON array. Most entries are directly playable channels, but
two (`p4`, `p5`) are containers whose actual stations live in a nested
`districts` array rather than at the top level.

| Field                | Notes                                                                 |
| --------------------- | ----------------------------------------------------------------------- |
| `slug`                 | Stable identifier — used as `provider_id`                              |
| `title`                | Station name                                                           |
| `description`          | May be absent; drop if empty                                           |
| `audioAssets[]`        | List of stream variants (`format: "HLS"` or `"ICY"`); prefer HLS       |
| `channelLogos[]`       | Dedicated square (`ratio: "1:1"`) logo, present on most channels       |
| `imageAssets[]`        | General images; occasionally includes a `1:1` crop as a fallback logo |
| `districts[]`          | Present only on `p4` and `p5` — each element has the same shape as a top-level channel |

A top-level entry with a non-empty `districts` array (`p4`, `p5`) is a
container, not a station — skip it and emit one station per district
instead (`P4 Bornholm`, `P4 Fyn`, etc.).

### Logo URL

`channelLogos`/`imageAssets` entries only carry an `id`, not a direct URL.
Construct it:

```
https://api.dr.dk/radio/v2/images/{id}?ratio=1:1
```

This redirects (302) to the actual cropped, square (e.g. 1080x1080) image —
follow the redirect as normal.

### Excluded channels

The API also returns several internal monitoring/test feeds that are not
real public stations, hardcoded as a skip-list: `mcrweb1`, `mcrweb2`,
`dr-web-2`, `dr-web-3`, `dr-web-4`, `dr-web-8`, `p3webcam`. These have generic
titles (`Mcrweb1`, `WEB3`, etc.), several are duplicate aliases of the same
underlying stream, and some have no `presentationUrl` at all — they're not
meant for a public directory.

Two other entries, `p7mix` and `special-radio`, have neither `audioAssets`
nor `districts` and are skipped automatically (no stream to resolve).

## Data Points

| Field          | Source                              | Notes                                             |
| -------------- | ------------------------------------- | -------------------------------------------------- |
| `name`         | `title`                                |                                                    |
| `stream_url`   | `audioAssets[]`, HLS preferred         | Falls back to the first listed (ICY/MP3) variant   |
| `logo_url`     | `channelLogos[]` or `imageAssets[]`    | Constructed square-crop URL; `None` if no `1:1` entry exists |
| `country`      | Hardcoded                             | Always `Denmark` / `DK`                            |
| `description`  | `description`                         | Drop if empty                                      |
| `provider_id`  | `slug`                                 |                                                    |
| `trusted`      | Hardcoded                             | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **Single request** returns the full channel lineup, including regional
  districts nested under `p4`/`p5`.
- **A container entry has no stream of its own.** Check `districts` before
  looking for `audioAssets` on a top-level channel.
- **Filter out the internal test feeds explicitly** — the API does not mark
  them as such (no `type` or flag distinguishes them from real stations).

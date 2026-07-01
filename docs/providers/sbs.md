# SBS Provider

SBS (Special Broadcasting Service) is Australia's multilingual/multicultural
public broadcaster — sister to ABC, a separate organisation. Its "Front Of
Site" service backs the live audio player and returns everything needed for
the registry (name, stream URL, description, logo) in a single call.

## Station Discovery

### Channels endpoint

```
GET https://fos.sbs.com.au/web/audio/channels?ids={bspId1},{bspId2},...
```

`ids` accepts a comma-separated batch of Brightspot CMS UUIDs — one call
covers every channel. Returns a plain JSON array.

| Field                | Notes                                                              |
| --------------------- | ---------------------------------------------------------------------- |
| `epgId`                | Stable slug, e.g. `sbs-chill` — used as `provider_id`                  |
| `name`                 | Station display name, e.g. `SBS Chill`                                 |
| `description`          | Short description                                                     |
| `streamUrl`            | Direct HLS URL — **served over plain HTTP**, not HTTPS                 |
| `leadImage.attributes.sizes[]` | Multiple entries by `name` (`16x9`, `1x1`, `4x3`); all point at the same SVG in practice, but `1x1` is the semantically-correct one to use |

### No channel-list endpoint

There's no API that enumerates all SBS channel IDs. The `bspId` (Brightspot
UUID) per channel was found by inspecting each channel page's
server-rendered data (`__next_f` SSR payload) rather than any public
directory. A known eighth channel — SBS EuroPop / Sounds of Home, HLS slug
`sbs4` — has a confirmed-live stream but no findable current page or ID, so
it's excluded here.

### Stream URL

`streamUrl` is returned as `http://`, not `https://`. This is passed through
as-is — the pipeline's liveness check already upgrades to HTTPS where
possible and falls back to HTTP otherwise, so no special handling is needed
in the provider.

## Data Points

| Field          | Source                          | Notes                                             |
| -------------- | ---------------------------------- | -------------------------------------------------- |
| `name`         | `name`                              |                                                    |
| `stream_url`   | `streamUrl`                         | HTTP, not HTTPS — see above                        |
| `logo_url`     | `leadImage.attributes.sizes[]`, `1x1` entry |                                             |
| `country`      | Hardcoded                          | Always `Australia` / `AU`                          |
| `description`  | `description`                       | Drop if empty                                      |
| `provider_id`  | `epgId`                             |                                                    |
| `trusted`      | Hardcoded                          | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required**, CORS-open (`*`).
- **Batch all channel IDs in one request** — `ids` is comma-separable and
  the endpoint returns an array in one response, matching how SBS's own
  player fetches them.
- **`streamUrl` is HTTP, not HTTPS.** Don't assume all stream URLs from an
  API are HTTPS-ready.

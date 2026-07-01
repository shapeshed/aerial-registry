# CBC Provider

CBC (Canadian Broadcasting Corporation) is Canada's English-language public
broadcaster. Its full live radio lineup — CBC Radio One's regional stations
and CBC Music — is available through a single public, unauthenticated
endpoint that already includes resolved stream URLs.

Reference: [rrradio's broadcaster research](https://github.com/MarkusSteinbrecher/rrradio/blob/244e5f1e8559237c2e08ea65f4130796e8a25bbd/docs/broadcaster-research.md#cbc--cbc--radio-canada-ca)
documents CBC/Radio-Canada's separate now-playing/schedule APIs, which aren't
used here — this provider only needs the one station-directory endpoint,
which happens to already carry the resolved stream URL.

## Station Discovery

### Live streams endpoint

```
GET https://www.cbc.ca/listen/api/v1/live-radio/live-streams
```

Returns `{"data": [...]}`, 43 entries at time of writing — no pagination.

| Field                | Notes                                                              |
| --------------------- | ---------------------------------------------------------------------- |
| `fullTitle`            | Ready-to-use display name, e.g. `CBC Radio One: Kenora`                |
| `description`          | Short description, e.g. `Radio One Kenora Live`                        |
| `streamUrl`            | Direct Akamai HLS URL — **already resolved**, no separate call needed |
| `media.callSign`       | Stable identifier, e.g. `CBC_R1_TOR` — used as `provider_id`           |
| `network.logoUrl`      | Wordmark logo, shared across all stations on that network             |

Unlike RAI/RTP/NRK, there is no per-station resolution step — `streamUrl` is
already the final playable URL in the same response that lists the stations.
(The doc also mentions a `services.radio-canada.ca/media/validation/v2`
resolver keyed by a numeric `idMedia` — that's how CBC's own site resolves
these `streamUrl`s server-side, but since the result is already in this
response, there's no need to call it separately.)

### Scope: English CBC only

CBC (English) and Radio-Canada (French, `ici.radio-canada.ca/ohdio`) share
the same Akamai stream infrastructure but are otherwise separate web stacks
with no shared discovery endpoint. This provider covers only English CBC —
**38 CBC Radio One regional stations and 5 CBC Music regional stations**.

French Radio-Canada (ICI Première, ICI Musique) is **out of scope**: its
streams resolve through the same `media/validation/v2` endpoint given a
numeric `idMedia`, but there is no public directory mapping French station
names/regions to their `idMedia` values (unlike the English side, which
gets this for free via `live-streams`). Discovering it would mean walking
an undocumented ID range with no confirmation of what a given ID resolves
to — not attempted here.

## Data Points

| Field          | Source                | Notes                                             |
| -------------- | ------------------------ | -------------------------------------------------- |
| `name`         | `fullTitle`               |                                                    |
| `stream_url`   | `streamUrl`               | Already resolved, direct HLS                       |
| `logo_url`     | `network.logoUrl`         | Wordmark, shared across a network's stations        |
| `country`      | Hardcoded                | Always `Canada` / `CA`                             |
| `description`  | `description`             | Drop if empty                                      |
| `provider_id`  | `media.callSign`          |                                                    |
| `trusted`      | Hardcoded                | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **No stream resolution step needed** — a rarity among the region-heavy
  providers in this registry (contrast RAI, RTP, NRK, ABC).
- **French Radio-Canada is a separate, unimplemented surface.** Don't
  assume this provider's `provider_id` (English `callSign`) format applies
  there — it uses `networkId`/`regionId` pairs instead.

# NPO Provider

NPO (Nederlandse Publieke Omroep) is the Dutch public broadcasting
organisation. Its radio channel metadata is available through a public
GraphQL API, but the actual playback URL for that API's channels is
geo-restricted to the Netherlands — this provider instead pairs that API's
channel metadata with NPO's older, still-live, unrestricted Icecast streams.

## Station Discovery

### GraphQL endpoint

Introspection is disabled, and there's no published schema — the query below
was reconstructed by exploiting GraphQL's "did you mean" suggestions on
invalid field names to iteratively discover valid ones.

```
POST https://api.nporadio.nl/graphql
{"query": "query { core_channels { data { id name slug } } }"}
```

Returns 28 entries covering NPO's radio *and* TV channels, its podcast/VOD
platforms (`npo-luister`, `npo-start`), and a catch-all `other` entry, with
no field distinguishing radio from the rest.

### Why the modern stream API isn't used

Each channel also exposes `live_video { mid }`, a media ID (e.g.
`LI_RADIO1_300877`) intended for NPO's standard playback flow:

```
GET  https://npo.nl/start/api/domain/player-token?productId={mid}   -> { jwt }
POST https://prod.npoplayer.nl/stream-link
     Authorization: {jwt}
     {"profileName": "hls", "referrerUrl": "https://npo.nl/"}
```

This **returns HTTP 451** ("Dit programma mag niet bekeken worden vanaf jouw
locatie" — "this programme may not be viewed from your location") for both
`hls` and `dash` profiles from outside the Netherlands. Since the registry is
built from GitHub Actions runners (not NL-based), this flow cannot be used.

### Icecast streams

NPO's legacy Icecast infrastructure (`icecast.omroep.nl`, now CNAMEd to
`icecast.npocloud.nl`) is still live and **not geo-restricted**. Its slugs
don't follow a predictable transform from the GraphQL `slug` field, so a
manual mapping table is maintained in the provider:

```
https://icecast.omroep.nl/{icecastSlug}
```

e.g. GraphQL slug `npo-radio-1` → Icecast slug `radio1-bb-mp3`.

Only channels with a confirmed-working Icecast slug are included (15 of the
28 GraphQL entries). This naturally excludes all TV channels, the VOD/podcast
platforms, and the catch-all `other` entry — but it also excludes a few real
radio channels that aren't on this legacy infrastructure and have no other
unrestricted stream source: **NPO Soul & Jazz, NPO Sterren NL, NPO Campus,
FunX Fissa, FunX Afro**.

## Data Points

| Field          | Source                        | Notes                                             |
| -------------- | ------------------------------- | -------------------------------------------------- |
| `name`         | `name`                           |                                                    |
| `stream_url`   | Icecast slug mapping             | See above; not derivable from the GraphQL slug     |
| `logo_url`     | Not available                    | No image field exists on `core_channels`; always `None` |
| `country`      | Hardcoded                       | Always `Netherlands` / `NL`                        |
| `provider_id`  | GraphQL `slug`                   |                                                    |
| `trusted`      | Hardcoded                       | `true` — broadcaster-direct, skip liveness checks |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required** for the GraphQL channel list or the Icecast
  streams.
- **The obvious/modern stream resolution path is geo-blocked.** Don't try to
  wire up `prod.npoplayer.nl` — it will 451 in CI.
- **The Icecast slug mapping is a hardcoded, manually-verified table**, not
  derived from any API field. If NPO retires this legacy infrastructure, the
  provider will need a different stream source entirely.

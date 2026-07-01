# Rinse Provider

Rinse is a UK-based electronic/bass music broadcaster with three sibling
stations sharing infrastructure, plus a French offshoot on separate
infrastructure. There is no public API for station discovery — the live
channel lineup is embedded as Craft CMS entry data inside a React Server
Components payload on `rinse.fm`'s homepage, and no REST/GraphQL endpoint
under `rinse.fm/api` or `rinse.fm/graphql` responds (all guessed paths 404
after redirect).

## Station Discovery

### No usable API

The homepage's `self.__next_f.push(...)` RSC payload embeds a `channelData`
object with one `channel_Entry` per station (`title`, `slug`,
`streamerMountPoint`, brand colour, social links). This is the only place
the four live channels and their stream URLs are listed. Parsing an RSC wire
payload reliably in Rust isn't worthwhile for four stations that don't
change often, so this provider hardcodes the list instead — the same
approach used for RTVE.

### Stations

| Station        | Stream host                          | Notes                                    |
| --------------- | -------------------------------------- | ------------------------------------------ |
| Rinse FM        | `admin.stream.rinse.fm/proxy/rinse_uk/stream` |                                     |
| Kool FM         | `admin.stream.rinse.fm/proxy/kool/stream`     | Drum & bass / jungle station              |
| SWU FM          | `admin.stream.rinse.fm/proxy/swu/stream`      | Reggae / dub / hip-hop station            |
| Rinse France    | `radio10.pro-fhi.net/flux-trmqtiat/stream`     | Separate infrastructure from the other three |

**The stream path changed** from the previously-curated `admin.stream.rinse.fm/stream`
(no `/proxy/{station}/` segment) to the current form — the old URL 404s.
Confirm against the live site if these break again rather than guessing a
new pattern.

### Logo

No verified logo source was found. The previously-curated Rinse FM logo
(`rinse.fm/wp-content/uploads/...`) 403s — the site migrated off WordPress
to Craft CMS/Next.js, and the whole path is gone. `logo_url` is `None` for
all four stations rather than guessing a URL that can't be confirmed to
exist.

## Data Points

| Field          | Source     | Notes                                             |
| -------------- | ------------ | -------------------------------------------------- |
| `name`         | Hardcoded    |                                                    |
| `stream_url`   | Hardcoded    | Verify against the live site if these ever break   |
| `logo_url`     | Not available | Always `None`                                     |
| `country`      | Hardcoded    | Always `United Kingdom` / `GB`, including Rinse France |
| `tags`         | Hardcoded    |                                                    |
| `trusted`      | Hardcoded    | `true` — broadcaster-direct, skip liveness checks |

Description is not available from this provider.

## API Behaviour Notes

- **No API — this is a static, hardcoded list**, same pattern as RTVE.
  Re-verify the stream URLs periodically by checking `rinse.fm`'s embedded
  channel data if streams stop resolving.
- **Rinse France uses different infrastructure** (`pro-fhi.net`, a French
  streaming host) from the other three (`admin.stream.rinse.fm`) — don't
  assume a shared base URL.

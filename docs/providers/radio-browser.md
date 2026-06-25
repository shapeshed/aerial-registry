# Radio Browser Provider

Radio Browser (<https://www.radio-browser.info>) is a community-maintained open
catalogue of internet radio stations with rich metadata including tags, country,
language, and codec. No authentication is required.

Radio Browser serves two roles in the registry pipeline:

1. **Standalone provider** — query top-voted stations to populate the registry
   with stations not covered by broadcaster-direct providers.
2. **Metadata enrichment source** — look up a favicon or tags for a station
   discovered from another provider.

## Server Discovery

Radio Browser runs multiple independent servers. Discover the live server list
before making any queries:

```
GET https://all.api.radio-browser.info/json/servers
```

Returns a JSON array of server objects. Each has a `name` field containing the
hostname. Shuffle the list and try each in order, failing over on error.

If the servers endpoint is unreachable, fall back to DNS: resolve all A records
for `all.api.radio-browser.info` and use the canonical hostnames.

## Station Discovery

### Top stations endpoint

Fetch the highest-voted stations as a bulk starting point:

```
GET https://{server}/json/stations?order=votes&reverse=true&limit=5000&hidebroken=true
```

Adjust `limit` based on how broad coverage should be. `hidebroken=true` omits
stations that have recently failed liveness checks on Radio Browser's own
infrastructure.

### Search endpoint

Search by name, useful when enriching records from other providers:

```
GET https://{server}/json/stations/search?name={query}&limit=200&order=votes&reverse=true&hidebroken=true
```

### Response fields

Each station object contains:

| Field          | Notes                                                            |
| -------------- | ---------------------------------------------------------------- |
| `stationuuid`  | Stable UUID for the station                                      |
| `name`         | Station name — may contain codec/bitrate noise, clean before use |
| `url_resolved` | Resolved stream URL. Required for imported stations.             |
| `favicon`      | Logo URL                                                         |
| `country`      | Full country name                                                |
| `countrycode`  | ISO 3166-1 alpha-2                                               |
| `tags`         | Comma-separated genre/topic tags                                 |
| `homepage`     | Station website URL                                              |
| `language`     | Comma-separated language names                                   |
| `votes`        | Community vote count                                             |
| `clickcount`   | Recent click count                                               |
| `codec`        | Audio codec e.g. `MP3`, `AAC`                                    |
| `bitrate`      | Stream bitrate in kbps                                           |

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

## Data Points

| Field          | Source         | Notes                                    |
| -------------- | -------------- | ---------------------------------------- |
| `name`         | `name`         | Clean codec/bitrate noise before storing |
| `stream_url`   | `url_resolved` | Required; skip records where this is empty |
| `logo_url`     | `favicon`      |                                          |
| `country`      | `country`      |                                          |
| `country_code` | `countrycode`  |                                          |
| `tags`         | `tags`         | Split on comma, lowercase, deduplicate   |
| `description`  | —              | Not available                            |
| `provider_id`  | `stationuuid`  | Stable UUID; useful for deduplication    |

## API Behaviour Notes

- **No authentication required.**
- **User-Agent header.** Radio Browser asks clients to identify themselves.
  Include a descriptive `User-Agent` header on all requests.
- **Server failover.** If a server returns an error, mark it as recently failed
  and try the next server in the discovered list before giving up.
- **`hidebroken=true`** omits stations Radio Browser has already detected as
  dead. The pipeline liveness checker will catch any remaining dead streams.

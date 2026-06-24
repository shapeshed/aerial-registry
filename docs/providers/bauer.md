# Bauer Provider

Bauer operates the Planet Radio API, which covers all of its UK and Irish radio
brands including Absolute Radio, KISS, Magic, heat, and Scala Radio. No
authentication is required.

## Station Discovery

### Stations endpoint

There is no countries endpoint to enumerate supported territories
programmatically. The country codes to query must be hardcoded. Verified as of
2026-06-24:

- `/stations/GB` — 44 UK stations
- `/stations/IE` — 27 Irish stations

The bare `/stations` endpoint and `/stations/AU` both return the same 44
stations with identical stream URLs as `/stations/GB` and can be ignored.

Fetch the station list for each known country:

```
GET https://listenapi.planetradio.co.uk/api9.2/stations/GB
GET https://listenapi.planetradio.co.uk/api9.2/stations/IE
```

Each response is a JSON array. Each element represents one station. The fields
relevant to the registry are:

| Field                        | Notes                                     |
| ---------------------------- | ----------------------------------------- |
| `stationName`                | Human-readable station name               |
| `stationStreams[].streamUrl` | Array of stream URLs; use the first entry |
| `stationListenBarLogo`       | Logo URL                                  |

Skip any station with no `stationName` or no resolvable stream URL.

### Country

Country is inferred from the endpoint queried:

| Endpoint | Country        | Country code |
| -------- | -------------- | ------------ |
| `/GB`    | United Kingdom | GB           |
| `/IE`    | Ireland        | IE           |

### Stream URL note

Bauer stream URLs require an `aw_0_1st.skey` query parameter at playback time
(the current Unix epoch in seconds). The registry stores the bare URL from the
API response. The consuming application must append
`?aw_0_1st.skey={epoch_seconds}` (or `&aw_0_1st.skey={epoch_seconds}` if the URL
already contains a query string) at the point of playback.

## Data Points

| Field          | Source                        | Notes                      |
| -------------- | ----------------------------- | -------------------------- |
| `name`         | `stationName`                 |                            |
| `stream_url`   | `stationStreams[0].streamUrl` | Bare URL, no skey appended |
| `logo_url`     | `stationListenBarLogo`        |                            |
| `country`      | Inferred from endpoint        | See country table above    |
| `country_code` | Inferred from endpoint        | See country table above    |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required.** The Planet Radio API is public.
- **User-Agent header.** Include a descriptive `User-Agent` header on all
  requests.
- **Multiple streams per station.** `stationStreams` may contain more than one
  entry. Take the first.

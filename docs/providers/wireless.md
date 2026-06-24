# Wireless Provider

Wireless Group operates a public API listing its UK stations. No authentication
is required for station discovery. Wireless owns talkSPORT, talkRADIO, Virgin
Radio UK, and Times Radio.

## Station Discovery

### Stations endpoint

```
GET https://talksport.com/play/api/stations
```

Returns a JSON array. Each element represents one station. The fields relevant
to the registry are:

| Field                 | Notes                                                        |
| --------------------- | ------------------------------------------------------------ |
| `name`                | Human-readable station name                                  |
| `streams.progressive` | Preferred stream URL                                         |
| `streams.hls`         | Fallback stream URL if `progressive` is absent               |
| `thumbnail`           | Logo — may be an object with a `url` field, or a bare string |
| `logo`                | Fallback logo — same dual shape as `thumbnail`               |

To extract the logo URL: check if the field is an object and read `.url`, or use
the value directly if it is a string. Try `thumbnail` first, fall back to
`logo`.

Skip any station with no `name` or no resolvable stream URL.

### Country

All Wireless stations are UK-based. Hardcode `United Kingdom` / `GB` for all
records from this provider.

## Data Points

| Field          | Source                | Notes                          |
| -------------- | --------------------- | ------------------------------ |
| `name`         | `name`                |                                |
| `stream_url`   | `streams.progressive` | Fall back to `streams.hls`     |
| `logo_url`     | `thumbnail` or `logo` | May be object or bare string   |
| `country`      | Hardcoded             | Always `United Kingdom` / `GB` |
| `country_code` | Hardcoded             | Always `GB`                    |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required** for station discovery.
- **User-Agent header.** Include a descriptive `User-Agent` header on all
  requests.

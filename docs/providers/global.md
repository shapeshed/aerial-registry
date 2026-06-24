# Global Player Provider

Global Radio exposes a public BFF (backend-for-frontend) API that lists all of
its stations. No authentication is required. Global owns Heart, Capital, LBC,
Classic FM, Radio X, Gold, and their regional variants.

## Station Discovery

### Stations endpoint

```
GET https://bff-web-guacamole.musicradio.com/stations/
```

Returns a JSON array of 149 stations (verified 2026-06-24). Each element
represents one station. The fields relevant to the registry are:

| Field              | Notes                                                           |
| ------------------ | --------------------------------------------------------------- |
| `name`             | Human-readable station name, e.g. `Capital London`              |
| `streamUrl`        | Primary stream URL                                              |
| `stream.icecastSd` | Fallback stream URL if `streamUrl` is absent                    |
| `tagline`          | Short station strapline, usable as a description                |
| `brand.slug`       | Brand identifier e.g. `capital`, `heart`. Used for logo lookup. |

The `stream` object also contains `icecastHd` and `hls` variants; prefer
`streamUrl` or `stream.icecastSd` for the widest device compatibility.

Skip any station with no `name` or no resolvable stream URL.

### Logo URL

The stations endpoint does not return a logo URL directly. The brand only
provides a `slug`. Use the Radio Browser API to look up a favicon by searching
for the brand slug:

```
GET https://{server}/json/stations/search?name={brand.slug}&countrycode=GB&limit=5&hidebroken=true
```

Take the first non-empty `favicon` from the results. See
`docs/providers/radio-browser.md` for Radio Browser server discovery.

### Country

All Global stations are UK-based. Hardcode `United Kingdom` / `GB` for all
records from this provider.

## Data Points

| Field          | Source                              | Notes                          |
| -------------- | ----------------------------------- | ------------------------------ |
| `name`         | `name`                              |                                |
| `stream_url`   | `streamUrl` or `stream.icecastSd`   |                                |
| `logo_url`     | Radio Browser favicon by brand slug | Secondary request required     |
| `description`  | `tagline`                           | Optional                       |
| `country`      | Hardcoded                           | Always `United Kingdom` / `GB` |
| `country_code` | Hardcoded                           | Always `GB`                    |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.** The BFF API is public.
- **User-Agent header.** Include a descriptive `User-Agent` header on all
  requests.
- **Regional variants.** Many brands have multiple regional entries, e.g.
  Capital Teesside, Capital London, Capital Manchester. These are distinct
  stations with distinct stream URLs and should each produce a registry entry.

# Global Player Provider

Global Radio owns Heart, Capital, LBC, Classic FM, Radio X, Smooth, Gold and
their regional variants. Global Player is a Next.js app; this provider
discovers the full live station list — national **and** regional — from the
app's published sitemap and data endpoints. No authentication is required.

## Station Discovery

The provider previously used a public BFF
(`bff-web-guacamole.musicradio.com/stations/`), which now returns `404` for
every path.

### 1. Build id

The build id changes on every deploy, so it is read from the homepage:

```
GET https://www.globalplayer.com/        → "buildId":"<id>"
```

### 2. Station list — the radio sitemap

The radio sitemap enumerates every station page, including regional variants
(`/live/capital/teesside/`, `/live/heart/kent/`, …):

```
GET https://www.globalplayer.com/sitemaps/sitemap_radio.xml
→ 144 × /live/{brand_slug}/{station_slug}/
```

### 3. Per-station data

Each station's page carries its name, logo and playback URLs:

```
GET https://www.globalplayer.com/_next/data/{buildId}/live/{brand_slug}/{station_slug}.json
→ pageProps.station  { name, brandLogo, tagline, gduid }
→ pageProps.playable.playback[]
```

Playback entries have `url` and `flags`. Subscriber entries carry
`auth.license` / `AdFree` (the `-plus` URLs) and the HD entry carries
`auth.HDAuth`; the **ad-supported** entry (`GlobalAdSupported`, served from
`media-ssl.musicradio.com`) is the plain public stream, which is the one
selected.

## Logos

Taken directly from `pageProps.station.brandLogo` — no secondary lookup. In a
live check, 142 of the 144 sitemap stations resolved with both a logo and a
public stream; the other two (`capital/rugby`, `capital/warwick`) are stale
sitemap entries that `308`-redirect to their canonical stations
(`capital/coventry`, `capital/stratford`) and collapse under dedup.

## Data Points

| Field          | Source                          |
| -------------- | ------------------------------- |
| `name`         | `station.name`                  |
| `stream_url`   | first public `playback[].url`   |
| `logo_url`     | `station.brandLogo`             |
| `description`  | `station.tagline`               |
| `country`      | constant — United Kingdom       |
| `country_code` | constant — `GB`                 |
| `provider_id`  | `station.gduid`, else `station.id` |

## API Behaviour Notes

- **No authentication required.** These are the endpoints the web app itself uses.
- **Discovery is live.** New stations, logos and stream URLs are picked up
  automatically; only the build id is fetched per run.
- **Fault-tolerant.** The per-station endpoint intermittently returns `5xx`
  under load, so requests are bounded (16 concurrent) and retried with
  exponential backoff. Only stations that still fail after retries (e.g. the
  two stale sitemap slugs that `308`-redirect to HTML) are skipped.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

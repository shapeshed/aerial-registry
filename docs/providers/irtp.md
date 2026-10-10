# IRTP (Peru) Provider

Radio Nacional del Perú is the public radio station of the Instituto Nacional
de Radio y Televisión del Perú (IRTP) — the country's first radio station, on
air since 1925.

Reference: <https://www.radionacional.gob.pe> · <https://www.irtpplay.gob.pe>

## Station Discovery

IRTP publishes **no station-list endpoint**: its site's Drupal JSON:API is
blocked (`403`), `radionacional.gob.pe` has no WordPress REST API, and the
player resolves the stream client-side. The station is therefore catalogued.

| Station              | Stream                                                                |
| -------------------- | --------------------------------------------------------------------- |
| Radio Nacional del Perú | `http://cdnhd.iblups.com/hls/0773874174fd4eba8bb9eff741d190dc.m3u8` |

> **Fragile URL.** The manifest path is a per-stream hash on IRTP's `iblups`
> CDN and is served over cleartext HTTP only (the host presents a TLS
> certificate for a different name, so HTTPS fails validation). It will need
> re-resolving if IRTP reissues the stream.

## Logos

The published mark from Wikimedia Commons:

```
https://upload.wikimedia.org/wikipedia/commons/3/36/Radio_Nacional_del_Per%C3%BA_%282021%29.svg
```

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | curated                       |
| `stream_url`   | curated                       |
| `logo_url`     | Commons mark                  |
| `country`      | constant — Peru               |
| `country_code` | constant — `PE`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | `radio-nacional`              |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** A single station.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

# RNV (Venezuela) Provider

Radio Nacional de Venezuela (RNV) is Venezuela's state radio service. Its
national channels include RNV Informativa, RNV Juvenil, RNV Musical and RNV
Activa.

Reference: <https://rnv.gob.ve> · <https://rnv.gob.ve/en-vivo>

## Station Discovery

RNV publishes no station-list endpoint. Its site is WordPress, but the stream
URLs are hard-coded into page content (`guri.tepuyserver.net/8048` and `/8156`)
with `[radio_player]` shortcodes; the host exposes no listing. The provider
therefore catalogues the two channels with a broadcaster-published stream:

| Station       | Stream                                       |
| ------------- | -------------------------------------------- |
| RNV Informativa | `https://guri.tepuyserver.net/8048/stream` |
| RNV Juvenil     | `https://guri.tepuyserver.net/8156/stream` |

RNV Musical and RNV Activa have no broadcaster-published stream URL and are not
included.

## Logos

RNV publishes a single brand mark, used for both:

```
https://rnv.gob.ve/wp-content/uploads/2026/05/Logo-RNV-210.png
```

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | curated — channel name        |
| `stream_url`   | curated                       |
| `logo_url`     | RNV brand mark                |
| `country`      | constant — Venezuela          |
| `country_code` | constant — `VE`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | channel slug                  |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** Adding a channel means adding a row to
  `STATIONS`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

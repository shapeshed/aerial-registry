# STVR (Slovakia) Provider

Slovenská televízia a rozhlas (STVR, formerly RTVS) is Slovakia's public
broadcaster. Its radio services are Rádio Slovensko, Rádio Regina (three
regional editions), Rádio Devín, Radio_FM, Rádio Klasika, Rádio Litera,
Rádio Patria, Radio Slovakia International and Rádio Junior.

Reference: <https://www.stvr.sk/radio/radia>

## Station Discovery

STVR streams from its own Icecast host, `live.slovakradio.sk:8000`, one mount
per service and bitrate (`<Service>_<32|128|256>.mp3`). Neither status document
gives a usable channel list:

- `status-json.xsl` is **malformed**: for a source with no now-playing title
  Icecast emits an unquoted `"title":-`, which is not valid JSON.
- `status.xsl` lists only the low-level mounts, with no service names.

The set is therefore pinned explicitly (like `trm`/`rtva`), using each
service's 128 kbps mount:

```
http://live.slovakradio.sk:8000/<mount>
```

| Service                     | Mount              |
| --------------------------- | ------------------ |
| Rádio Slovensko             | `Slovensko_128.mp3` |
| Rádio Regina Bratislava     | `Regina_BA_128.mp3` |
| Rádio Regina Banská Bystrica| `Regina_BB_128.mp3` |
| Rádio Regina Košice         | `Regina_KE_128.mp3` |
| Rádio Devín                 | `Devin_128.mp3`     |
| Radio_FM                    | `FM_128.mp3`        |
| Rádio Klasika               | `Klasika_128.mp3`   |
| Rádio Litera                | `Litera_128.mp3`    |
| Rádio Patria                | `Patria_128.mp3`    |
| Radio Slovakia International| `RSI_128.mp3`       |
| Rádio Junior                | `Junior_128.mp3`    |

The `Rozhlasova_rada` mount is STVR's council feed, not a station, and is
excluded.

## Streams are HTTP-only

The host refuses TLS on `:8000` and serves nothing on `:443`, so stream URLs
are cleartext HTTP. This mirrors the broadcaster's own player, which uses the
same URLs.

## Logos

STVR publishes one SVG per service under
`https://www.stvr.sk/media/images/radiostations/`. Rádio Klasika has no
published mark here and carries the STVR logo.

## Data Points

| Field          | Source                          |
| -------------- | ------------------------------- |
| `name`         | curated — service name          |
| `stream_url`   | `{STREAM_BASE}/{mount}`         |
| `logo_url`     | service SVG, else STVR logo     |
| `country`      | constant — Slovakia             |
| `country_code` | constant — `SK`                 |
| `tags`         | empty; left to enrichment       |
| `description`  | none                            |
| `provider_id`  | mount                           |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** Adding a service means adding a row to
  `STATIONS`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

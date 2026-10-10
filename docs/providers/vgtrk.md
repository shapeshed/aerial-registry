# VGTRK (Russia) Provider

The All-Russia State Television and Radio Broadcasting Company (VGTRK) is
Russia's national broadcaster. Its radio services are Радио России (Radio
Rossii), Радио Маяк, Радио Культура and Вести FM.

**Caveat.** VGTRK is a **state broadcaster**, not an independent public-service
one. Russia has no equivalent of the BBC/SRF/NPO, so unlike the other national
providers in this repository there is no editorially independent public
broadcaster to draw from — this is the only national source available.

Reference: <https://vgtrk.ru> · <https://smotrim.ru>

## Station Discovery

VGTRK's stations stream from a single Icecast host, `icecast.vgtrk.cdnvideo.ru`,
one mount per service. There is **no channel-list endpoint**, so the set is
pinned explicitly (like `trm`/`rtva`):

| Service       | Mount                     |
| ------------- | ------------------------- |
| Радио России  | `rrzonam_mp3_192kbps`     |
| Радио Маяк    | `mayakfm_mp3_192kbps`     |
| Радио Культура| `kulturafm_mp3_192kbps`   |
| Вести FM      | `vestifm_mp3_192kbps`     |

## Streams are HTTP-only

The host serves **cleartext HTTP only** — TLS returns nothing — so stream URLs
are `http://icecast.vgtrk.cdnvideo.ru/<mount>`.

Reachability was verified live from a UK network (Slough, GB): all four mounts
answered `200 audio/mpeg`, so they are not geoblocked from the UK. As they are
broadcaster-direct, the provider is marked **trusted** and skips liveness
probing; even if a build runner elsewhere were refused, the liveness
classification treats 403/451 and timeouts as `Inconclusive` and keeps the
station.

## Logos

Published marks from Wikimedia Commons for Радио России, Радио Маяк and Вести
FM. No published mark was found for Радио Культура, which carries no
`logo_url`.

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | curated — Russian service name |
| `stream_url`   | `{STREAM_BASE}/{mount}`       |
| `logo_url`     | Commons mark, else none       |
| `country`      | constant — Russia             |
| `country_code` | constant — `RU`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | mount                         |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** Adding a service means adding a row to
  `STATIONS`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

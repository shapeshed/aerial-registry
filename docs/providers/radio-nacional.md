# Radio Nacional (Argentina) Provider

Radio Nacional is the public radio network of Argentina, operated by Radio y
Televisión Argentina (RTA). It is a chain of ~57 stations: the Buenos Aires
flagship (LRA1, AM 870) plus four national FM services (Clásica, Folklórica,
Rock, RAE) and a network of regional emisoras (`LRA`/`LT`/`LU`/`LV` callsigns)
across every province.

Reference: <https://www.radionacional.com.ar> · <https://es.wikipedia.org/wiki/Radio_Nacional_Argentina>

## Station Discovery

There is **no public station-list endpoint**. RTA does not publish the network
as machine-readable data:

- The site's WordPress REST API is disabled — every `/wp-json/*` path returns
  `401 rest_login_required`.
- The broadcaster's own stream host exposes no directory: the Icecast
  `status-json.xsl` is unreachable and `status.xsl` / the server root both
  require authentication.
- The web player at `/reproductor/` is not reachable from outside RTA's network.

Because discovery is impossible, this provider **catalogues the known mounts**
(the same approach as `trm`, `rtva` and `vatican-radio`). The set was derived
by probing the Icecast host for `sc_radN` mounts (1–120) and keeping the ones
that answered, then cross-checking each against the broadcaster's published
emisora list and the mount's own `icy-name` / `icy-description` headers.

```
GET https://sa.mp3.icecast.magma.edge-access.net/sc_rad{mount}
```

The host serves unsigned MP3 over HTTP/1.1 (`Content-Type: audio/mpeg`); no
tokens, referrer checks or cookies are involved.

### National services

| Station                    | Mount     |
| -------------------------- | --------- |
| Radio Nacional Buenos Aires | `sc_rad1` |
| Nacional Clásica           | `sc_rad37` |
| Nacional Folklórica        | `sc_rad38` |
| Nacional Rock              | `sc_rad39` |
| RAE Argentina al Mundo     | `sc_rad35` |

### Regional emisoras

Regional mounts are `sc_radN` where `N` generally tracks the `LRA` station
number, but not always (for example `sc_rad31` carries Gualeguaychú/LRA42 and
`sc_rad102` carries Jáchal/LRA51). The mapping is pinned explicitly in the
provider rather than derived from the number.

Mounts that did not answer — including LRA16 (La Quiaca), LRA19 (Puerto
Iguazú) and LRA25 (Tartagal) — are omitted. `sc_rad110` ("Control Central") is
an internal technical feed and is deliberately excluded.

## Logos

RTA's player serves per-service logos for the national channels:

| Service      | Logo                                                        |
| ------------ | ---------------------------------------------------------- |
| Buenos Aires | `https://cdn.radionacional.com.ar/reproductor/images/Logo_AM870_b.png`     |
| Clásica      | `https://cdn.radionacional.com.ar/reproductor/images/Logo_Clasica.png`     |
| Folklórica   | `https://cdn.radionacional.com.ar/reproductor/images/Logo_Folklorica.png`  |
| Rock         | `https://cdn.radionacional.com.ar/reproductor/images/Logo_Rock.png`        |
| RAE          | `https://cdn.radionacional.com.ar/reproductor/images/Logo_RAE.png`         |

Every regional station shares the current Radio Nacional mark:

```
https://cdn.radionacional.com.ar/wp-content/uploads/2025/06/LogoRadioNacional3.png
```

There is no per-regional-station artwork to fetch.

## Data Points

| Field          | Source        | Notes                                            |
| -------------- | ------------- | ------------------------------------------------ |
| `name`         | curated       | `Radio Nacional <City>`, or the service's brand  |
| `stream_url`   | mount URL     | `{STREAM_BASE}/{mount}`                          |
| `logo_url`     | curated       | Per-service for national; shared mark otherwise  |
| `country`      | constant      | Argentina                                        |
| `country_code` | constant      | `AR`                                             |
| `tags`         | —             | Empty; left to enrichment                        |
| `description`  | —             | None                                             |
| `provider_id`  | mount         | e.g. `sc_rad1`                                   |

## API Behaviour Notes

- **No authentication required** for playback.
- **Catalogue, not discovery.** Adding a new LRA station means probing the host
  for a new mount and adding a row to `STATIONS`.
- **Trusted.** These are RTA's own broadcaster-direct mounts, so the provider
  is marked trusted and skips liveness probing.

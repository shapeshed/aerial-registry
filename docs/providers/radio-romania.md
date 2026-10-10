# Radio România (Romania) Provider

Radio România is the national public radio service of Romania, operated by
Societatea Română de Radiodifuziune (SRR). It runs the national networks
(Actualități, Cultural, Muzical, Antena Satelor, București FM), the RRI
international service, and a network of regional stations.

Reference: <https://www.radioromania.ro> · <https://www.srr.ro>

## Station Discovery

SRR's streaming Icecast host publishes its own status document, so this
provider discovers the live channel list at runtime rather than cataloguing a
fixed set:

```
GET https://stream4.srr.ro:8443/status-json.xsl
```

The document is a standard Icecast `status-json.xsl`: an `icestats.source`
array (or single object) with a `listenurl`, `server_name` and `listeners` per
mount. No authentication is required.

The status document advertises cleartext `:8008` URLs
(`http://stream4.srr.ro:8008/<mount>`). The same mounts are served over TLS on
`:8443`, which is what the provider stores:

```
https://stream4.srr.ro:8443/<mount>
```

### Duplicate names

SRR occasionally lists one service on two mounts — for example Radio Reșița on
`radio-resita` (AAC, busier) and `radio-resita2` (MP3). The provider keeps the
busiest mount per `server_name` so the app does not show the same station
twice.

## Logos

SRR publishes one plain SVG per service under
`https://www.romania-actualitati.ro/templates/default/images/`. The mount slug
does not map mechanically onto the file name, so the mapping is explicit
(`logo_for`). Antena Sibiului, Radio Sighet and the Junior channels publish no
mark and are left without `logo_url`.

## Data Points

| Field          | Source                                  |
| -------------- | --------------------------------------- |
| `name`         | `server_name`, trimmed                  |
| `stream_url`   | `{STREAM_BASE}/{mount}`                 |
| `logo_url`     | curated per-mount SVG                   |
| `country`      | constant — Romania                      |
| `country_code` | constant — `RO`                         |
| `tags`         | empty; left to enrichment               |
| `description`  | none                                    |
| `provider_id`  | mount                                   |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** New mounts appear without a code change; only their
  logo needs adding to `logo_for`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

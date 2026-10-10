# RTVC (Colombia) Provider

RTVC (Sistema de Medios Públicos) is Colombia's national public broadcaster. Its
radio services are Radio Nacional de Colombia, Radiónica and the digital
Exploremos.

Reference: <https://www.rtvc.gov.co> · <https://www.radionacional.co>

## Station Discovery

RTVC's "Parrilla" site (`parrilla.rtvc.gov.co`) is a Drupal back end that drives
its channel lineup and exposes the standard Drupal JSON:API. Each channel node
carries an HLS URL in `field_streaming_hls` and a logo file:

```
GET https://parrilla.rtvc.gov.co/jsonapi/node/channel?page%5Blimit%5D=50&include=field_logo_json
```

The same list contains the television channels (Señal Colombia, Canal
Institucional, RTVC Noticias) and the radio-studio webcams. RTVC names its
**radio** streams under `Radio_`, so the provider selects the three radio
channels by that path segment and ignores the rest:

| Station                     | `field_streaming_hls`                                                        |
| --------------------------- | ---------------------------------------------------------------------------- |
| Radio Nacional de Colombia  | `…/Radio_Radionacional/Radionacional.stream/playlist.m3u8`                   |
| Radiónica                   | `…/Radio_Radionica/Radionica.stream/playlist.m3u8`                           |
| Exploremos                  | `…/Radio_Digital/Radionacional_Digital.stream/playlist.m3u8`                 |

Logos are resolved from the channel's `field_logo_json` file relationship via the
JSON:API `included` block (Exploremos has none).

## Data Points

| Field          | Source                                    |
| -------------- | ----------------------------------------- |
| `name`         | `title`                                   |
| `stream_url`   | `field_streaming_hls`                     |
| `logo_url`     | `field_logo_json` file URL, else none     |
| `country`      | constant — Colombia                       |
| `country_code` | constant — `CO`                           |
| `tags`         | empty; left to enrichment                 |
| `description`  | none                                      |
| `provider_id`  | `drupal_internal__nid`                    |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** New radio channel nodes appear without a code change.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

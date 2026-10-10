# RNP (Paraguay) Provider

Radio Nacional del Paraguay (RNP) is Paraguay's state radio service. It
broadcasts on 920 AM and 95.1 FM from Asunción, with additional services on
700 AM and 105.9 FM (San Pedro).

Reference: <http://webaudio.radionacional.gov.py/> · <https://www.radionacional.gov.py>

## Station Discovery

RNP's live-player page runs WordPress with the "Radio Player" plugin, which
embeds each station's configuration as a base64 `data-data` attribute:

```html
<div data-data="eyJ0aXRsZSI6IjkyMEFNIiwi…" data-player-type="shortcode"></div>
```

Decoded, each blob is JSON:

```json
{ "stations": [ { "title": "RNP 920 AM",
    "stream": "http://audio.radionacional.gov.py/920",
    "thumbnail": "http://webaudio.radionacional.gov.py/wp-content/uploads/2024/01/920.jpg" } ] }
```

The provider fetches the page and decodes the configs, so the station list is
**discovered** rather than catalogued:

| Station     | Stream                                          |
| ----------- | ----------------------------------------------- |
| RNP 920 AM  | `http://audio.radionacional.gov.py/920`          |
| RNP 95.1 FM | `http://audio2.radionacional.gov.py/951fm`       |
| RNP 700 AM  | `http://audio.radionacional.gov.py/700am`        |
| RNP 105.9 FM| `http://audio2.radionacional.gov.py/sanpedro`    |

Streams are RNP's own, served over cleartext HTTP.

## Logos

The plugin's `thumbnail` field, under
`webaudio.radionacional.gov.py/wp-content/uploads/2024/01/`.

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | `title`                       |
| `stream_url`   | `stream`                      |
| `logo_url`     | `thumbnail`                   |
| `country`      | constant — Paraguay           |
| `country_code` | constant — `PY`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | slugified title               |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** New player configs appear without a code change.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

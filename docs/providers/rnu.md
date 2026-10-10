# RNU (Uruguay) Provider

Radiodifusión Nacional del Uruguay (RNU) is Uruguay's state radio service. Its
national stations are Radio Uruguay, Radio Clásica, Radio Cultura and Radio
Babel.

Reference: <https://mediospublicos.uy/category/radio/>

## Station Discovery

RNU publishes one "… en Vivo" page per station on its WordPress site, each
embedding an iwstreaming player widget whose id maps to a stream on the same
host:

```
https://mediospublicos.uy/…/player/single?p=8036   →   https://radios.iwstreaming.uy/8036/stream
```

The WordPress REST API lets the provider **discover** the station list by
searching page content for the widget, rather than cataloguing the four
stations:

```
GET https://mediospublicos.uy/wp-json/wp/v2/pages?search=iwstreaming&per_page=100&_fields=slug,title,content
```

| Station       | Widget id | Stream                                    |
| ------------- | --------- | ----------------------------------------- |
| Radio Uruguay | 8036      | `https://radios.iwstreaming.uy/8036/stream` |
| Radio Clásica | 8032      | `https://radios.iwstreaming.uy/8032/stream` |
| Radio Cultura | 8034      | `https://radios.iwstreaming.uy/8034/stream` |
| Radio Babel   | 8030      | `https://radios.iwstreaming.uy/8030/stream` |

The iwstreaming host is a Uruguayan streaming provider; the streams are RNU's
own.

## Logos

From RNU's WordPress uploads, mapped by station name:

- Radio Uruguay — `…/2026/06/radio_uruguay.png`
- Radio Clásica — `…/2026/06/radio_clasica.png`
- Radio Cultura — `…/2026/06/radio_cultura.png`
- Radio Babel — `…/2021/06/radio-babel.png`

## Data Points

| Field          | Source                                   |
| -------------- | ---------------------------------------- |
| `name`         | page title, minus " en Vivo"             |
| `stream_url`   | `https://radios.iwstreaming.uy/{id}/stream` |
| `logo_url`     | mapped by name                           |
| `country`      | constant — Uruguay                       |
| `country_code` | constant — `UY`                          |
| `tags`         | empty; left to enrichment                |
| `description`  | none                                     |
| `provider_id`  | page slug                                |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** New "… en Vivo" pages appear without a code change;
  only a new station's logo needs adding to `logo_for`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

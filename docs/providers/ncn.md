# NCN (Guyana) Provider

The National Communications Network (NCN) is Guyana's state-owned radio and
television broadcaster. Its radio services include Voice of Guyana (VOG),
98.1 Hot FM and Vybz 100.1 FM, plus regional stations.

Reference: <https://ncnguyana.com>

## Station Discovery

NCN's radio landing page lists each station with its logo, name and a
`listen.php?station=<CODE>` link; the linked page embeds the stream in an
`<audio><source>`. The provider walks that two-level structure:

```
GET https://ncnguyana.com/radio.php          → logo, name, code
GET https://ncnguyana.com/listen.php?station=<CODE> → <source src="…">
```

Streams are served from `cast4.asurahosting.com/proxy/<id>/stream`. The
published stations are:

```
VOG HOTFM VYBZ RMABARUMA RESSEQUIBO ROREALLA
RBARTICA RMAHDIA RLETHEM RAISHALTON RPAIWOMAK
```

Logos are the page's `<img src>` values, resolved against `ncnguyana.com`.

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | listing name                  |
| `stream_url`   | `listen.php` `<source>`       |
| `logo_url`     | listing `<img>`               |
| `country`      | constant — Guyana             |
| `country_code` | constant — `GY`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | station code                  |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** New stations added to `radio.php` appear automatically.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

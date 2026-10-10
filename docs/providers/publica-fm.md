# Pública FM (Ecuador) Provider

Pública FM is the national public radio of Ecuador, run by COMEP (Medios
Públicos) — the successor to Radio Pública del Ecuador.

Reference: <https://www.publicafm.ec>

## Station Discovery

Pública FM's site is a JavaScript app, but its server-rendered configuration
carries the stream URL as a `streamingUrl` field (HTML- and JSON-escaped in the
page source). The provider reads it, so the stream is **discovered** rather than
catalogued:

```
GET https://www.publicafm.ec/
→ "streamingUrl":"https://comep.radioca.st/stream"
```

| Station    | Stream                                |
| ---------- | ------------------------------------- |
| Pública FM | `https://comep.radioca.st/stream`     |

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | constant — Pública FM         |
| `stream_url`   | `streamingUrl`                |
| `logo_url`     | none available                |
| `country`      | constant — Ecuador            |
| `country_code` | constant — `EC`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | `publica-fm`                  |

## API Behaviour Notes

- **No authentication required.**
- **Discovery is live.** A changed `streamingUrl` is picked up automatically.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

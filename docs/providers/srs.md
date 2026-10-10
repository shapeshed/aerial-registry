# SRS (Suriname) Provider

Stichting Radio-omroep Suriname (SRS) is Suriname's state-owned radio service,
broadcasting since 1965 on 96.3 FM (Paramaribo, Wanica, Saramacca, Commewijne)
and several regional frequencies.

Reference: <https://radiosrs.sr>

## Station Discovery

SRS publishes **no station-list endpoint**, and its own site (`radiosrs.sr`) is
behind bot protection from the build location. Its stream is served by its
streaming host, SuriLive, and the stream's ICY headers identify it as SRS
(`icy-name: Stichting Radio-omroep Suriname - SuriLive.com`,
`icy-url: https://radiosrs.sr`). The service is therefore catalogued:

| Station    | Stream                        |
| ---------- | ----------------------------- |
| Radio SRS  | `https://surilive.com:8060/;` |

## Logos

The SRS mark from the Surinamese radio directory:

```
https://radio.sr/assets/srs.jpg
```

## Data Points

| Field          | Source                        |
| -------------- | ----------------------------- |
| `name`         | curated — Radio SRS           |
| `stream_url`   | curated                       |
| `logo_url`     | SRS mark                      |
| `country`      | constant — Suriname           |
| `country_code` | constant — `SR`               |
| `tags`         | empty; left to enrichment     |
| `description`  | none                          |
| `provider_id`  | `srs-963`                     |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** A single service.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

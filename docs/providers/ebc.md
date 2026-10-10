# EBC (Brazil) Provider

Empresa Brasil de Comunicação (EBC) is Brazil's public broadcasting company. Its
radio stations are the Rádio Nacional network (Rio de Janeiro, São Paulo,
Brasília AM/FM, São Luís, Amazônia, Alto Solimões) and Rádio MEC / MEC FM.

Reference: <https://radionacional.ebc.com.br> · <https://radiomec.ebc.com.br>

## Station Discovery

EBC's sites are Plone, and every station is an `Emissora` content object whose
REST representation carries the HLS `stream_url`. The station list is therefore
**discovered from the broadcaster's own API** rather than catalogued:

```
GET https://{site}/++api++/@search?portal_type=Emissora&b_size=100&metadata_fields=stream_url
```

with `{site}` one of:

- `radionacional.ebc.com.br` — the Rádio Nacional network (7 stations)
- `radiomec.ebc.com.br` — Rádio MEC AM / MEC FM (2 stations)

The search response includes the fields the provider needs directly — `title`,
`stream_url` and `image_scales` — so no per-object follow-up request is
required:

```json
{
  "@id": "https://radionacional.ebc.com.br/emissoras/radio-nacional-do-rio-de-janeiro",
  "title": "Rádio Nacional do Rio de Janeiro",
  "stream_url": "https://radionacionalrio-stream.ebc.com.br/ebc/radionacionalriodejaneiro/playlist.m3u8",
  "image_scales": { "image": [ { "download": "@@images/image-563-….png",
      "scales": { "preview": { "download": "@@images/image-400-….png" } } } ] }
}
```

The streams are unsigned HLS served from per-network EBC hosts
(`*-stream.ebc.com.br`).

## Logos

Taken from the object's Plone image scales (`preview`, falling back to `thumb`
and then the full image), resolved against the object `@id`.

## Data Points

| Field          | Source                                   |
| -------------- | ---------------------------------------- |
| `name`         | `title`                                  |
| `stream_url`   | `stream_url`                             |
| `logo_url`     | `image_scales` (absolute-ised)           |
| `country`      | constant — Brazil                        |
| `country_code` | constant — `BR`                          |
| `tags`         | empty; left to enrichment                |
| `description`  | none                                     |
| `provider_id`  | last path segment of `@id`               |

## API Behaviour Notes

- **No authentication required.** `++api++` is Plone's standard REST endpoint.
- **Discovery is live.** New `Emissora` objects appear without a code change.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

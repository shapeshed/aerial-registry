# Radio France Provider

Radio France is the French public broadcasting group. Its stations are available
via the Radio France Open API GraphQL endpoint. Authentication requires an API
key issued via the developer portal at <https://developers.radiofrance.fr/>.

API endpoint: `https://openapi.radiofrance.fr/v1/graphql`

## Authentication

Pass the API key as an HTTP header on every request:

```
x-token: <RADIO_FRANCE_API_KEY>
```

The pipeline reads the key from the `RADIO_FRANCE_API_KEY` environment variable.
If the variable is absent or empty the provider logs an error and returns an
empty list — it does not abort the pipeline.

## Station Discovery

### GraphQL query

```graphql
{
  brands {
    id
    title
    baseline
    description
    liveStream
    webRadios {
      id
      title
      description
      liveStream
    }
    localRadios {
      id
      title
      description
      liveStream
    }
  }
}
```

### Response structure

Each element of `brands` represents a top-level Radio France brand (France
Inter, France Musique, FIP, etc.) and may contain three types of stream:

| Field         | Description                                                        |
| ------------- | ------------------------------------------------------------------ |
| `liveStream`  | The main brand stream (may be `null` for umbrella brands like ICI) |
| `webRadios`   | Thematic sub-channels, e.g. FIP Rock, FIP Jazz, Classique Easy     |
| `localRadios` | Regional stations in the ICI / France Bleu network                 |

All three are emitted as separate registry entries. Skip any entry whose
`liveStream` is `null` or empty.

### Station naming

Sub-radio titles are prefixed with the parent brand title when they do not
already begin with it (case-insensitive):

| Brand           | Sub-radio title      | Registry name                  |
| --------------- | -------------------- | ------------------------------ |
| FIP             | FIP Rock             | FIP Rock                       |
| France Musique  | Classique Easy       | France Musique - Classique Easy |
| ICI             | ICI Alsace           | ICI Alsace                     |
| ICI             | 100% chanson française | ICI - 100% chanson française  |

### Description

For brand-level entries use `baseline` as the description (short tagline),
falling back to `description` if `baseline` is absent. For sub-radios use
`description`.

## Data Points

| Field         | Source                        | Notes                                             |
| ------------- | ----------------------------- | ------------------------------------------------- |
| `name`        | `title` / derived             | See naming rules above                            |
| `stream_url`  | `liveStream`                  | Direct Icecast MP3 URL                            |
| `logo_url`    | Brand page `og:image`         | Fetched from `radiofrance.fr/{brand_slug}`; slug = `brand.id.to_lowercase()`. The pikapi image server accepts `{width}x{height}` — social-share dimensions are rewritten to `500x500`. Sub-stations (web radios, local radios) share their parent brand's logo. |
| `country`     | Hardcoded                     | Always `France` / `FR`                            |
| `description` | `baseline` or `description`   | Short tagline preferred                           |
| `provider_id` | `id`                          | Stable Radio France identifier (e.g. `FRANCEINTER`, `FIP_ROCK`) |
| `trusted`     | Hardcoded                     | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider; the enrichment step queries Radio
Browser to add them.

## API Behaviour Notes

- **Single request.** All brands, web radios, and local radios are returned in
  one GraphQL call.
- **ICI brand has no top-level stream.** The `FRANCEBLEU` brand (`liveStream:
  null`) serves only as a container for regional local radios; skip its brand
  entry and emit only the individual `localRadios`.
- **Stream URL includes `?id=openapi`.** This suffix is provided by the API and
  should be preserved as-is.

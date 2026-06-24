# ARD Provider

ARD (Arbeitsgemeinschaft der öffentlich-rechtlichen Rundfunkanstalten der
Bundesrepublik Deutschland) is the German public broadcasting consortium. Its
stations are accessible via the ARD Audiothek GraphQL API with no authentication
required.

API endpoint: `https://api.ardaudiothek.de/graphql`

## Station Discovery

### GraphQL query

Send a POST request with the following query to retrieve all permanent
livestreams:

```graphql
{
  permanentLivestreams(first: 200) {
    nodes {
      id
      title
      audios {
        url
        mimeType
      }
      image {
        url
      }
      publicationService {
        genre
      }
    }
  }
}
```

The response is a JSON object with a `data.permanentLivestreams.nodes` array.
Each node represents one radio station.

### Stream URL selection

Each node has an `audios` array containing one or more stream variants with
different MIME types. Select the stream in this priority order:

1. `application/vnd.apple.mpegurl` — HLS adaptive stream (preferred)
2. First available entry if HLS is absent

Skip any node with no audio entries.

### Logo URL

Each node has an `image.url` field that contains a template with a `{width}`
placeholder, e.g.:

```
https://images.ardaudiothek.de/some-image/{width}/image.jpg
```

Replace `{width}` with `500` to obtain a usable logo URL.

### Genre / Tags

The `publicationService.genre` field provides a single genre string such as
`"Regional"`, `"Pop und Szene"`, `"Kultur"`, or `"Info"`. Use this as the sole
tag when present and non-empty.

## Data Points

| Field         | Source                     | Notes                                                           |
| ------------- | -------------------------- | --------------------------------------------------------------- |
| `name`        | `title`                    | Station name in German                                          |
| `stream_url`  | `audios`                   | HLS preferred, fallback to first entry                          |
| `logo_url`    | `image.url`                | Replace `{width}` with `500`                                    |
| `country`     | Hardcoded                  | Always `Germany` / `DE`                                         |
| `tags`        | `publicationService.genre` | Single genre string; omit if empty                              |
| `provider_id` | `id`                       | Stable ARD Audiothek identifier                                 |
| `trusted`     | Hardcoded                  | `true` — broadcaster-direct, skip liveness checks               |

## API Behaviour Notes

- **No authentication required.** The GraphQL endpoint is fully public.
- **`first: 200` parameter.** The API currently returns around 197 stations;
  this parameter ensures all are fetched in one request.
- **Genre values are in German.** e.g. `"Pop und Szene"`, `"Nachrichten"`.
  These are passed through as-is; no translation is applied.

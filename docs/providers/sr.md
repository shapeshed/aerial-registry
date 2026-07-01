# Sveriges Radio (SR) Provider

Sveriges Radio is the Swedish public broadcaster. Its full channel lineup —
national, regional, minority-language, and digital-only — is available
through SR's public Open API. No authentication is required.

API docs: <https://api.sr.se/api/documentation/v2/index.html>

## Station Discovery

### Channels endpoint

```
GET https://api.sr.se/api/v2/channels?format=json&pagination=false
```

Returns an object with a `channels` array covering all live channels in one
request — national (`Rikskanal`), regional (`Lokal kanal`), minority-language,
and other digital-only channels. No pagination or per-channel resolution is
needed.

| Field            | Notes                                                          |
| ----------------- | --------------------------------------------------------------- |
| `id`               | Stable numeric channel ID — used as `provider_id`               |
| `name`             | Station name, e.g. `P1`, `P4 Blekinge`                          |
| `tagline`          | Short Swedish-language description                              |
| `image`            | Logo URL, already cropped square (`?preset=api-default-square`) |
| `liveaudio.url`     | Direct stream URL — redirects to the actual Icecast/HLS stream  |

Skip any entry with no `liveaudio.url`.

## Data Points

| Field          | Source            | Notes                                             |
| -------------- | ------------------ | -------------------------------------------------- |
| `name`         | `name`             |                                                    |
| `stream_url`   | `liveaudio.url`    | Redirects to the actual stream; no separate resolution step needed |
| `logo_url`     | `image`            | Already square, no cropping/transform needed       |
| `country`      | Hardcoded          | Always `Sweden` / `SE`                             |
| `description`  | `tagline`          | Drop if empty                                      |
| `provider_id`  | `id`               |                                                    |
| `trusted`      | Hardcoded          | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **Requires a `User-Agent` header.** SR's edge returns a 403 Access Denied
  for requests with no `User-Agent` at all (confirmed with a bare `reqwest`
  client) — the app's existing custom User-Agent avoids this, but it's worth
  noting if this provider is ever called from a different HTTP client.
- **Single request, no fan-out.** Unlike RAI/RTP, there is no per-channel
  resolution step — `liveaudio.url` is directly usable and stable.

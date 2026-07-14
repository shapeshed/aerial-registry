# FluxFM Provider

FluxFM (Berlin) exposes its full channel list as a single public JSON
endpoint. No authentication required.

## Station Discovery

```
GET https://fluxmusic.api.radiosphere.io/channels
```

Returns `{"items": [...]}`. Fields used: `channelId` (stable, used as
`provider_id`), `displayName`, `summary` (description, sometimes null),
`streams` (array of `{name, encoding, bitrate, url}` — already direct,
playable URLs, no playlist-wrapper resolution needed), `coverImages` (a map
of size label to URL, e.g. `"256_256.png"` — sometimes `null`, not just
absent, for a couple of channels).

`preferred_stream()` picks the highest-bitrate `mp3` entry, falling back to
the first stream listed.

Most channel names don't include the "FluxFM" brand (e.g. "80s",
"Clubsandwich"); `prefixed_name()` adds it unless the name already starts
with "Flux" (e.g. "FluxLounge").

## Mapping

- `provider = "fluxfm"`, `trusted = true` — this is the station's own
  official API.
- `country = "Germany"` / `country_code = "DE"` (FluxFM is Berlin-based).

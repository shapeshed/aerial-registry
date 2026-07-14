# SomaFM Provider

SomaFM publishes its full channel list as a single public JSON endpoint. No
authentication required.

## Station Discovery

```
GET https://somafm.com/channels.json
```

Returns `{"channels": [...]}`. Fields used:

| Field         | Notes                                                              |
| ------------- | ------------------------------------------------------------------|
| `id`          | Stable channel id, e.g. `beatblender`. Used as `provider_id`.      |
| `title`       | Display name.                                                      |
| `description` | Short one-line description.                                       |
| `genre`       | Pipe-separated tag list, e.g. `"ambient\|electronic"`.             |
| `largeimage`  | 256px logo.                                                        |
| `playlists`   | Array of `{url, format, quality}` — see below.                     |

## Stream URL resolution

Each `playlists` entry's `url` is a `.pls` playlist wrapper, not a directly
playable stream — it lists several redundant `ice*.somafm.com` mirrors for
one stream. `preferred_playlist()` picks the highest-bitrate MP3 entry
(falling back to any MP3, then to the first entry), then fetches that `.pls`
file and takes its first `File1=` line as `stream_url`. A channel whose
playlist can't be resolved is skipped for that run rather than shipped with
a broken URL.

## Mapping

- `provider = "somafm"`, `trusted = true` — this is the station's own
  official API.
- `country = "United States"` / `country_code = "US"` (SomaFM is San
  Francisco-based; the app has no per-channel geography to use instead).
- `tags` — `genre` split on `|`, lowercased.

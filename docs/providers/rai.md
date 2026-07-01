# RAI Provider

RAI is the Italian public broadcaster. Its live radio channels are listed on
RAI's on-demand audio platform, RaiPlaySound, which exposes a public,
unauthenticated JSON endpoint for the live channel lineup. Stream URLs must be
resolved per-channel through RAI's relinker service, also unauthenticated.

## Station Discovery

### Live channels endpoint

```
GET https://www.raiplaysound.it/dirette.json
```

Returns an object with a `contents` array. Each element is one live channel:

| Field                | Notes                                                        |
| -------------------- | ------------------------------------------------------------ |
| `uniquename`         | Stable RAI content identifier — used as `provider_id`        |
| `title`               | Station name, e.g. `Rai Radio 1`                             |
| `audio.url`           | A relinker URL containing the channel's `cont` ID            |
| `channel.logo`        | Relative path to the channel logo (see Logo URL below)       |

Extract the `cont` query parameter from `audio.url` — this is the ID needed to
resolve the actual stream (see below). Skip any entry missing `audio` or an
extractable `cont` ID.

### Stream URL resolution

RAI does not return a playable URL directly — `audio.url` points at a relinker
endpoint that must be called separately, once per channel, to get a signed
stream URL:

```
GET https://mediapolis.rai.it/relinker/relinkerServlet.htm?cont={contId}&output=45
```

`output=45` returns a plain-text XML body (no CDATA wrapping) with the stream
URL inside a `<url type="content">...</url>` tag:

```xml
<Mediapolis>
<url type="content">https://radiounoest-live.akamaized.net/hls/live/2032586/radiounoest/radiounoest/playlist.m3u8?auth=...&aifp=V001</url>
...
</Mediapolis>
```

**This must be called fresh at every discovery run.** The relinker signs a new
Akamai/MainStreaming auth token on every call — the URL embedded in a
previous run's registry will not necessarily still be valid. This mirrors the
BBC provider's manifest-resolution pattern.

### Logo URL

`channel.logo` points at RAI's white-on-transparent wordmark, intended to sit
on the channel's own brand-colour background (`-transparent.png` suffix). That
renders as a flat, illegible shape on any other background. Strip the suffix
to get the coloured variant at the same path:

```
logo.replace("-transparent.png", ".png")
```

Then prefix with `https://www.raiplaysound.it`.

### Country

Every live channel is Italian except one joint-venture channel, `Radio San
Marino`, which should be tagged `San Marino` / `SM` instead of `Italy` / `IT`.

## Data Points

| Field          | Source                              | Notes                                             |
| -------------- | ------------------------------------ | -------------------------------------------------- |
| `name`         | `title`                              |                                                    |
| `stream_url`   | Relinker resolution                  | Resolved fresh per run; see above                 |
| `logo_url`     | `channel.logo`, coloured variant     | See Logo URL above                                |
| `country`      | Hardcoded, per-channel               | `Italy`/`IT`, except `San Marino`/`SM`            |
| `provider_id`  | `uniquename`                         |                                                    |
| `trusted`      | Hardcoded                            | `true` — broadcaster-direct, skip liveness checks |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required** for either endpoint.
- **Resolve at discovery time, not once.** Caching a resolved relinker URL
  across runs risks shipping a stream URL with an expired auth token.
- **All requests concurrent.** Channels are fetched and resolved in parallel
  via `futures::future::join_all`, matching the BBC provider's approach.
- Confirmed working set as of writing: 15 live channels, including Radio 1/2/3,
  Isoradio, GR Parlamento, Radio 1 Sport, Radio 3 Classica, Techetè, Kids, Live
  Napoli, Tutta Italiana, Trst A, Südtirol, No Name Radio, and Radio San Marino.

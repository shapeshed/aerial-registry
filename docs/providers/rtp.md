# RTP Provider

RTP (Rádio e Televisão de Portugal) is the Portuguese public broadcaster. Its
official mobile API requires a bearer token obtained by HMAC-SHA256-signing a
request with a key reverse-engineered from RTP's Android app (see the
third-party [rtp-play-api](https://github.com/guipenedo/rtp-play-api) project
for that approach). This provider deliberately avoids porting that scheme.
Instead it uses a plain, unauthenticated JSON endpoint that RTP's own website
already calls to render each channel's live page — no signing required.

## Station Discovery

### Per-channel on-air endpoint

```
GET https://www.rtp.pt/play/livechannelonair.php?channel={id}&howmanynext=1&howmanybefore=0&channeltype=radio
```

Returns `{"raw": {"result": [...]}}` — take the first (only) element of
`result`:

| Field                       | Notes                                                        |
| --------------------------- | ------------------------------------------------------------ |
| `channel_type`              | Must start with `radio` — the ID space is shared with TV channels; skip otherwise |
| `channel_name`              | Station name, e.g. `RTP Antena 1`                            |
| `channel_summary`           | Description; may contain ` ` (nbsp) that should be normalised to a plain space |
| `channel_card_logo`         | Coloured logo URL; can be an empty string rather than absent — filter accordingly |
| `channel_rewrite`           | Stable slug, e.g. `antena1` — used as `provider_id`          |
| `stream_url.http.standard`  | Direct, unauthenticated HLS playlist URL                     |

### Channel ID list

RTP has no endpoint that enumerates all radio channel IDs — TV and radio share
one ID space, and no public listing distinguishes them cleanly (the
`EPG/json/rtp-home-page/list-channels/radio` endpoint uses a *different*,
unrelated ID space that does not work against `livechannelonair.php`). The
current radio channel IDs were found by probing the ID space directly and
checking `channel_type`:

| ID  | Channel                |
| --- | ----------------------- |
| 91  | RTP Antena 1            |
| 92  | RTP Antena 2            |
| 1   | RTP Antena 3            |
| 94  | RTP África              |
| 95  | RTP Mundo (RDP Internacional) |
| 97  | RTP Antena1 Açores      |
| 98  | RTP Antena1 Madeira     |
| 99  | RTP Antena3 Madeira     |
| 100 | RTP Lusitânia           |
| 101 | RTP Jazzin              |
| 102 | Antena3 Dance           |
| 103 | RTP Vida                |
| 104 | Brasil 200              |

This list is maintained as a hardcoded const in the provider. If RTP adds a
new radio channel it will need a new ID added here manually — there is no
dynamic enumeration.

## Data Points

| Field          | Source                       | Notes                                             |
| -------------- | ----------------------------- | -------------------------------------------------- |
| `name`         | `channel_name`                |                                                    |
| `stream_url`   | `stream_url.http.standard`    | Direct HLS, no auth, no expiring token             |
| `logo_url`     | `channel_card_logo`           | Filter out empty strings                           |
| `country`      | Hardcoded                     | Always `Portugal` / `PT`                           |
| `description`  | `channel_summary`             | Normalise nbsp to space; drop if empty             |
| `provider_id`  | `channel_rewrite`             |                                                    |
| `trusted`      | Hardcoded                     | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required.** This is the same endpoint RTP's own website
  calls to render `rtp.pt/play/direto/{channel}` — it needs no HMAC signing or
  bearer token, unlike the official mobile API.
- **`channeltype=radio` is not a strict filter.** The endpoint returns
  whatever channel matches the numeric ID regardless of the `channeltype`
  query param — always check `channel_type` in the response before accepting
  a result.
- **All 13 channels fetched concurrently** via `futures::future::join_all`.
- If fewer stations resolve than expected, the provider logs an error (not a
  panic) — a channel ID may have been retired or repurposed.

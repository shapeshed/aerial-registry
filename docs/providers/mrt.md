# MRT (North Macedonia) Provider

Makedonska Radio Televizija (MRT) is North Macedonia's public broadcaster. Its
radio arm runs four national channels: Македонско Радио 1, 2 and 3, and the
satellite service Македонско Радио Сат.

Reference: <https://play.mrt.com.mk> · <https://mrt.com.mk>

## Station Discovery

MRT's player (`play.mrt.com.mk`) streams each radio channel as HLS from
`interspace.com`, behind a short-lived signed token:

```
https://vod-c57.interspace.com:443/channel_abr/<id>/playlist.m3u8?wmsAuthSign=…
```

The signature carries a 30-minute validity window, and the player's own inline
config (`gxArCurrPlaylist`) carries the current signed URL and a poster. The
URL therefore **must be resolved at every discovery run** — as with RAI's
relinker — rather than stored statically.

The provider fetches each channel's live page and extracts the manifest and
poster from that config:

```
GET https://play.mrt.com.mk/live/<slug>
```

| Service               | Slug       | Channel |
| --------------------- | ---------- | ------- |
| Македонско Радио 1    | `radio1`   | 47      |
| Македонско Радио 2    | `radio2`   | 48      |
| Македонско Радио 3    | `radio3`   | 49      |
| Македонско Радио Сат  | `radio-sat`| 50      |

## Data Points

| Field          | Source                             |
| -------------- | ---------------------------------- |
| `name`         | curated — Macedonian brand         |
| `stream_url`   | signed HLS URL from the live page  |
| `logo_url`     | `poster` from the live page        |
| `country`      | constant — North Macedonia         |
| `country_code` | constant — `MK`                    |
| `tags`         | empty; left to enrichment          |
| `description`  | none                               |
| `provider_id`  | live page slug                     |

## API Behaviour Notes

- **No authentication required.**
- **Resolve at discovery time, not once.** Caching a signed URL would serve a
  dead link after 30 minutes.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

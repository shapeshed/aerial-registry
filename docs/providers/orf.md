# ORF Provider

ORF (Österreichischer Rundfunk) is the Austrian public broadcaster. There is
no published API documentation — this provider was built from the config
bundle its own Sound web app loads on startup, cross-referenced against the
[youtube-dl ORF extractor](https://github.com/ytdl-org/youtube-dl/blob/master/youtube_dl/extractor/orf.py)
for background (though that extractor targets an older, now-unused part of
ORF's infrastructure and isn't directly usable here).

## Station Discovery

### Bundle endpoint

```
GET https://orf.at/app-infos/sound/web/1.0/bundle.json?_o=sound.orf.at
```

Returns a large config object; the relevant part is `stations`, a map keyed
by station slug (`oe1`, `oe3`, `bgl`, ...) rather than an array.

| Field                      | Notes                                                          |
| --------------------------- | ------------------------------------------------------------------ |
| `name`                       | Station display name, e.g. `Ö1`, `Radio Burgenland`                |
| `liveStreamUrlTemplate`      | HLS URL containing a `{quality}` placeholder                       |

Entries with no `liveStreamUrlTemplate` are TV channels or the on-demand
archive (`tv`, `orf`, `orf1`, `orf2`, `orf3`, `archive`) — skip them. This
leaves 14 real radio stations: `oe1`, `oe3`, `fm4`, the 9 regional Ö2
stations (`bgl`, `ktn`, `noe`, `ooe`, `sbg`, `stm`, `tir`, `vbg`, `wie` — one
per Austrian federal state), `campus`, and `slo`.

The bundle also has a top-level `privates` array of third-party partner
stations (Arabella, City23, energy, etc.) that ORF's Sound app aggregates for
discovery. **These are not ORF stations and are out of scope** — only the
keyed `stations` map is used.

### Stream URL resolution

`liveStreamUrlTemplate` looks like:

```
https://orf-live-oe1.mdn.ors.at/out/u/oe1/{quality}/manifest.m3u8
```

The `{quality}` placeholder isn't documented anywhere. It was found by
downloading ORF Sound's web app JS bundle (`sound.orf.at`, find the current
hashed bundle filename from the page's `<script type="module" src="...">`
tag) and grepping for literal quoted strings near the stream template
definitions. Two values exist: `q1a` (low bitrate, ~53kbps average) and
`q2a` (high bitrate, ~96kbps average). This provider uses `q2a`.

No further per-station resolution or authentication is needed — substitute
the literal string and the URL is directly playable.

### Logo

Not available. Unlike the `privates` partner stations (which do have a
`https://orf.at/app-infos/sound/logos/{slug}.png` pattern), ORF's own
station entries carry only background colours, not a logo image URL.

## Data Points

| Field          | Source                              | Notes                                             |
| -------------- | ------------------------------------- | -------------------------------------------------- |
| `name`         | `name`                                 |                                                    |
| `stream_url`   | `liveStreamUrlTemplate`                | `{quality}` replaced with `q2a`                    |
| `logo_url`     | Not available                          | Always `None`                                      |
| `country`      | Hardcoded                             | Always `Austria` / `AT`                            |
| `provider_id`  | Station slug (the map key)             |                                                    |
| `trusted`      | Hardcoded                             | `true` — broadcaster-direct, skip liveness checks |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required.**
- **`stations` is a map, not an array** — iterate its values, not indices.
- **Filter on `liveStreamUrlTemplate` presence**, not on any explicit "is
  this radio" flag — none exists. TV/archive entries simply omit the field.
- **Ignore the `privates` array** — it's third-party stations bundled into
  the same app for discovery, not ORF's own broadcasts.

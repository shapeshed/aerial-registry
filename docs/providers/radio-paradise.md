# Radio Paradise Provider

Radio Paradise's channel metadata is served from a Strapi CMS API. No
authentication required.

## Station Discovery

```
GET https://vsh-sdata.radioparadise.com/api/channels?populate=banner&pagination[pageSize]=50
```

Returns `{"data": [{"attributes": {...}}]}`. Fields used: `name`, `chan_id`,
`summary` (description), `banner.data.attributes.url` (a path relative to
`https://vsh-sdata.radioparadise.com`, not a full URL).

## Stream URL resolution

This API has no stream URL field at all. The direct stream host
(`stream.radioparadise.com`) follows a stable naming convention that's been
documented and used by third-party players for years — confirmed here
against community-submitted Radio Browser entries for the same channels
rather than assumed. It's keyed on `chan_id` in `STREAM_SLUGS`
(`src/providers/radio_paradise.rs`) since a channel's slug doesn't always
match its display name (Main Mix's stream is `aac-320`, not `main-320`).

Two channels have no known direct stream and are skipped: "My Favorites"
(chan_id 99, a personalised aggregate, not a broadcast) and "Mellow X"
(chan_id 4, no working slug found).

A per-session, geo-routed URL of the form
`https://audio-geo.radioparadise.com/chan/<id>/x/<instance>/...` also exists
(used by Radio Paradise's own web/app player) but isn't used here — the
instance id in that path rotates, so it isn't safe to hardcode.

## Mapping

- `provider = "radio-paradise"`, `provider_id` = `chan_id` as a string,
  `trusted = true` — this is the station's own official metadata.
- `country = "United States"` / `country_code = "US"` (Radio Paradise is
  California-based; there's no per-channel geography to use instead).

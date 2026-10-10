# RTS (Serbia) Provider

Radio Television of Serbia (RTS) is Serbia's state-owned public broadcaster.
Its radio arm, Radio Beograd, runs the national stations Beograd 1, Beograd 2,
Beograd 3 and Beograd 202.

Reference: <https://www.rts.rs> · <https://www.rts.rs/lat/radio/live.html>

## Station Discovery

RTS's radio player (`rts-radio.spectar.tv`, a Spectar OTT instance) reads a
small, public XML document listing every radio channel:

```
GET https://rts-radio.spectar.tv/channels.xml
```

Each `<channel>` block carries the HLS manifest (`<url>`), a poster image
(`<poster>`), a thumbnail (`<img>`) and an availability flag (`<available>`):

```xml
<channel>
    <id>15918</id>
    <name>Радио Београд 1</name>
    <url>https://rtsradio-live.morescreens.com/RTS_2_001/playlist.m3u8</url>
    <poster>https://static.rtsplaneta.rs/client_api.php/image/transform/…</poster>
    <available>1</available>
</channel>
```

The document contains four channels. The channels RTS lists on its site as
digital spin-offs (Pletenica, Rok, Džuboks, Vrteška, Džezer) are **not** in
this document, and the wider RTSPlaneta client API returns `401` without a key,
so four is the full set this endpoint exposes. Only channels with
`<available>1</available>` are imported.

The document is flat with one occurrence of each child tag, so it is parsed
with a small field extractor rather than an XML dependency.

## Data Points

| Field          | Source                          |
| -------------- | ------------------------------- |
| `name`         | `<name>`                        |
| `stream_url`   | `<url>` (HLS playlist)          |
| `logo_url`     | `<poster>`, else `<img>`        |
| `country`      | constant — Serbia               |
| `country_code` | constant — `RS`                 |
| `tags`         | empty; left to enrichment       |
| `description`  | none                            |
| `provider_id`  | `<id>`                          |

## API Behaviour Notes

- **No authentication required** for the channel list or the manifests.
- **HLS.** Streams are `playlist.m3u8` manifests on `rtsradio-live.morescreens.com`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

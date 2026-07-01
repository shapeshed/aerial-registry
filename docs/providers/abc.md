# ABC Provider

ABC (Australian Broadcasting Corporation) has no public API for radio station
discovery or stream resolution. Its player is a Next.js SPA (`abc.net.au`)
that embeds each station's stream URLs and logo directly in the page's
`__NEXT_DATA__` JSON blob — this provider scrapes that.

Reference: [rrradio's broadcaster research](https://github.com/MarkusSteinbrecher/rrradio/blob/244e5f1e8559237c2e08ea65f4130796e8a25bbd/docs/broadcaster-research.md#abc--australian-broadcasting-corporation-au)
documents ABC's separate now-playing/EPG metadata APIs (`music.abcradio.net.au`,
`program.abcradio.net.au`) and the `papiServiceId` taxonomy used here, but
those APIs don't return stream URLs — only track/programme metadata.

## Station Discovery

### Live pages

```
GET https://www.abc.net.au/listen/live/{slug}
```

Each page's HTML contains a `<script id="__NEXT_DATA__" type="application/json">`
block. Somewhere inside it (position varies by page — don't assume a fixed
path) is an object identified by having both a `papiServiceId` field and a
`config.sources` array:

```json
{
  "papiServiceId": "TRIPLEJ",
  "title": "triple j",
  "config": {
    "sources": [
      { "file": "https://streaming.abc-cdn.net.au/audio/hls/triplejnsw.m3u8?source=web" },
      { "file": "https://live-radio01.mediahubaustralia.com/2TJW/aac/?source=web" },
      { "file": "https://live-radio01.mediahubaustralia.com/2TJW/mp3/?source=web" }
    ]
  },
  "radioHeadingPrepared": {
    "logoPrepared": {
      "imgSrc": "https://live-production.wcms.abc-cdn.net.au/{hash}?impolicy=wcms_crop_resize&cropH=150&cropW=150&width=862&height=862"
    }
  }
}
```

`config.sources[0]` is consistently the HLS variant across every station
checked — use it directly rather than falling back to the AAC/MP3 entries.

**`title` is the current programme name, not the station's brand** (e.g. it
reads "Sleep Through" or "Overnights" outside of a specifically-branded
slot) — don't use it as the station name. Station names are hardcoded
alongside their slugs instead.

### No station-list endpoint

There is no API that enumerates all ABC radio stations. The CMS behind
`abc.net.au/listen/radio` groups them into an accordion ("Find your national
network", "Find your local station" broken out by state) backed by
CoreMedia collection IDs, but the actual station cards are lazy-loaded
client-side rather than present in the page's initial data — not worth
reverse-engineering for a one-off list. Station slugs were instead found by
following links between live pages and probing plausible city names.

### Scope

Covers every national network (11) plus the state-capital ABC Local stations
(9): Sydney, Melbourne, Brisbane, Adelaide, Perth, Hobart, Canberra, Darwin,
Newcastle. **Deliberately excludes ABC's further ~50 regional/rural Local
stations** (e.g. Gippsland, Riverland, Kimberley) — there's no discovery path
better than guessing individual city-name slugs, and that doesn't scale to a
one-off research pass.

## Data Points

| Field          | Source                                      | Notes                                             |
| -------------- | ---------------------------------------------- | -------------------------------------------------- |
| `name`         | Hardcoded, keyed by slug                        | The page's own `title` field is a programme name, not a station name |
| `stream_url`   | `config.sources[0].file`                        | Always the HLS variant                             |
| `logo_url`     | `radioHeadingPrepared.logoPrepared.imgSrc`      | Square-cropped; absent for one station (NewsRadio) |
| `country`      | Hardcoded                                       | Always `Australia` / `AU`                          |
| `provider_id`  | `papiServiceId`                                 | e.g. `TRIPLEJ`, `LOCAL_SYDNEY`                     |
| `trusted`      | Hardcoded                                       | `true` — broadcaster-direct, skip liveness checks |

Tags and description are not available from this provider.

## API Behaviour Notes

- **This is HTML scraping, not an API** — more fragile than other providers.
  If ABC changes their Next.js build or player component structure, the
  `__NEXT_DATA__` shape (and the `find_player_config` search predicate) may
  need updating. The recursive search-by-shape approach was chosen
  specifically to reduce sensitivity to depth/position changes within the
  data tree, but a rename of `papiServiceId` or `config.sources` would still
  break it.
- **One HTTP request per station** (20 total) — each returns a full HTML
  page, heavier than a typical JSON API call.
- **`title` in the player config is the current programme, not the brand.**
  This tripped up initial exploration — verify against a hardcoded name list
  rather than trusting page content for station identity.

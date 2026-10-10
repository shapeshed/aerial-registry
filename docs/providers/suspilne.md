# Suspilne (Ukraine) Provider

Suspilne (JSC "National Public Broadcasting Company of Ukraine", UA:PBC) runs
Ukraine's public radio: Ukrainian Radio (UR-1), Radio Promin, Radio Kultura,
Radio Ukraine International and the online Radiotochka service, plus the
Kultura spin-offs Kazka and Klasyka and the regional Radio Tysa.

Reference: <https://ukr.radio> · <https://corp.suspilne.media>

## Station Discovery

There is **no channel-list endpoint**. Suspilne's player is a JavaScript app
(`ukr.radio`) whose bundle carries no stream URLs: each channel's mounts are
read from its own page (`ukr.radio/channel.html?channelID=N`), which embeds
`radio.ukr.radio/<mount>` references. The stream host itself is Icecast:

```
GET https://radio.ukr.radio/status.xsl
```

but that document lists ~90 low-level mounts — every regional opt-out of
Ukrainian Radio on `ur1-<region>-*` plus the national services — so it is not a
usable channel list. This provider **catalogues the eight national services**
the channel pages expose, using each channel's MP3 mount:

```
https://radio.ukr.radio/<mount>
```

| Service                    | Mount           |
| -------------------------- | --------------- |
| Українське Радіо           | `ur1-mp3`       |
| Радіо Промінь              | `ur2-mp3`       |
| Радіо Культура             | `ur3-mp3`       |
| Radio Ukraine International | `ur4-mp3`       |
| Радіоточка                 | `ur5-mp3`       |
| Радіо Культура Казка       | `urkazka-mp3`   |
| Радіо Культура Класика     | `urclassic-mp3` |
| Радіо Тиса                 | `tysafm-mp3`    |

The regional `ur1-*` opt-outs are deliberately out of scope — they are windows
of Ukrainian Radio, not separate stations.

## Logos

Suspilne bundles its channel marks client-side and they are not separately
addressable (`ukr.radio/assets/*.svg` is a catch-all redirect, and
`suspilne.media`'s meta images are behind bot protection). The provider uses the
published marks from Wikimedia Commons for the five main services
(Ukrainian Radio, Promin, Kultura, RUI, Radiotochka); the Kazka/Klasyka
spin-offs and Radio Tysa have no separable mark and carry the shared Suspilne
logo, as RTA's regional stations carry the Radio Nacional mark.

## Data Points

| Field          | Source                          |
| -------------- | ------------------------------- |
| `name`         | curated — Ukrainian endonym     |
| `stream_url`   | `{STREAM_BASE}/{mount}`         |
| `logo_url`     | Commons mark, else shared logo  |
| `country`      | constant — Ukraine              |
| `country_code` | constant — `UA`                 |
| `tags`         | empty; left to enrichment       |
| `description`  | none                            |
| `provider_id`  | mount                           |

## API Behaviour Notes

- **No authentication required.**
- **Catalogue, not discovery.** Adding a service means adding a row to
  `STATIONS`.
- **Trusted.** Broadcaster-direct, so liveness probing is skipped.

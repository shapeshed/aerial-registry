# Hosting platforms as a discovery source

**Status: recorded, not implemented.** This is the highest-yield source we have
found, written down so it is not lost while scanner work continues. It is not
part of the current build.

## The signal

We do not need to *search* for hosting providers offering Icecast — they are
already visible in our own registry. Of 61,847 stations, **460 domains hold 20
or more stations each**, and the top of that list is nothing but platforms:

| domain | stations | what it is |
| --- | --- | --- |
| `stream.zeno.fm` | 1,567 | Zeno.fm — free hosting platform |
| `playerservices.streamtheworld.com` | 1,335 | StreamTheWorld — commercial CDN |
| `stream.laut.fm` | 1,200 | laut.fm — German hosting platform |
| `ice.infomaniak.ch` | 802 | Infomaniak — Swiss host |
| `streaming.live365.com` | 234 | Live365 |
| `listen.radioking.com` | 187 | RadioKing |
| `stream.radiojar.com` | 147 | Radiojar |
| `out.airtime.pro` | 129 | Airtime / Sourcefabric |

This is a **better signal than ASN density**, for two reasons:

1. It is **independent of the hosts we already know**, so it does not suffer the
   circularity that affects ASN density (where our known hosts predict our known
   hosts).
2. Platforms are **directories**. Some publish their whole catalogue over a
   public JSON API, so we get stream URLs *and* metadata directly — no scanning,
   no port list, no `/status-json.xsl`.

## Platform hosts are proxied

I tested 20 platform hosts for `/status-json.xsl` (zeno.fm, laut.fm,
infomaniak, streamtheworld, live365, radioking, radiojar, airtime, hearthis.at,
slotex, uber.radio, and more): **none expose it.** They are all behind CDN or
proxy front-ends, so the "one query, hundreds of mounts" trick does not apply,
and scanning their address space is pointless. Their value is purely the
directory/API.

## laut.fm — the biggest single win available

`https://api.laut.fm/stations` returns **16,748 stations**, free, no auth, 38 MB
of JSON. We currently hold 1,200 laut.fm stations via Radio-Browser, so this is
roughly **15,500 net-new**.

Keys: `name`, `display_name`, `spoken_name`, `page_url`, `stream_url`,
`api_urls`, `genres`, `location`, `lat`, `lng`, `images`, `description`, `djs`,
`active`, `position`, `format`, `updated_at`.

`https://zeno.fm/api/stations/` also works — paginated public JSON.

### What is on it

A **German self-service radio platform**: a long tail of hobby stations around a
themed core.

Genres (of the 8,980 that have any): Pop 3,233 · Rock 2,396 · Charts 1,450 ·
**Schlager 1,230** · 80er 1,159 · 90er 1,066 · Oldies 995 · Rap 918 ·
Hip-Hop 915 · Dance & Electronic 759 · Techno 751 · House 737 · Alternative 563 ·
Metal 490 · Trance 379 · Chillout 329 · Jazz 304 · Hardstyle 300.

Locations are Germany first (`Deutschland` 486, `Germany` 135) then German
cities (Berlin 417, Hamburg 218, Köln 149, München 117, Leipzig 97).

Station types: themed genre/decade channels (`Hardstyle FM`, `Synthwave`,
`Bluesrock`), an operator network of 38 `1000 …` stations (`1000 Oldies`,
`1000 Goldschlager`, `1000 Volksmusikhits`), and personal stations — **8,453 of
16,748 names are a single word** (`Cfr`, `Tiagosilva`, `simulator1`).

### Streams

ICY MP3, not HLS, with real bitrate available from the stream itself:

```
Content-Type: audio/mpeg
icy-name: Mega Sunshine
icy-genre: Rock
icy-audio-info: ice-channels=2;ice-samplerate=44100;ice-bitrate=128
```

So they belong on the `streams` variant path, not HLS ABR.

### Quality signals

| signal | verdict |
| --- | --- |
| `listeners` — `/station/<name>/listeners` | real, current integer, **one request per station**; no bulk endpoint (`?order=`/`?sort=` return malformed, truncated JSON) |
| `djs` | 9,011 stations (54 %) — human-curated / live shows |
| `description` | 57 % |
| `images` | artwork available |
| `genres` | 54 %, but 3,337 free-form tags — needs normalising |
| `active` | 16,747 / 16,748 — no filtering value |
| `position` | **not a ranking** (position 0 holds both a 25- and a 0-listener station) |
| `updated_at` | platform-wide bulk touch (14,646 at 2026-06) — not "maintained" |
| `format` | prose (`Musik für die Ewigkeit .`) — unusable |

Listener distribution over a 120-station random sample:

| | |
| --- | --- |
| median | 1 |
| mean | 1.8 |
| max | 26 |
| zero listeners | 28 % |
| ≥ 5 | 8 % |
| ≥ 10 | 1 % |
| ≥ 50 | 0 % |

The top stations reach 100–150 listeners; a random one has about one. A
listener threshold is therefore **brutally aggressive** — requiring ≥ 5 would
cut ~92 % of the catalogue. A single sample is also noisy, because listener
counts are instantaneous.

### Import considerations

1. **Genre normalisation** — 3,337 free-form tags ("80er"/"80s"/"80's") must map
   onto our vocabulary.
2. **Name cleaning** — `Cfr` and `simulator1` are not displayable station names.
3. **Deduplication** — the `1000 …` family and near-identical genre channels
   will collide; Radio-Browser already holds 1,200 of these.
4. **Liveness pruning is essential** — self-service means many are abandoned,
   and `active` will not tell us.
5. **Automated, not live-presenter** — most are playlist channels. If the app's
   mood/For-You features care about that distinction, tag laut differently from
   a broadcaster feed.

### Open decision

laut.fm is either a **large catalogue of near-silent stations**, or a **small
catalogue of ~1,300 genuinely-listened-to ones**. Prefer listeners as a ranking
signal (sampled more than once) rather than a hard filter, combined with
liveness and the `djs` flag.

## Where this sits in the strategy

The discovery hierarchy should read:

1. **Platform APIs / directories** ← this document
2. Broadcaster / provider APIs
3. ASN-targeted scanning — see `scanning.md`
4. Aggregators — Radio-Browser, Netlas, Xiph YP

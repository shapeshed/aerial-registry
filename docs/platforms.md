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

## AzuraCast — the highest-value enrichment path

AzuraCast is an open-source, self-hosted radio automation platform (liquidsoap
backend, Icecast or Shoutcast frontend). It is widely deployed, and it exposes a
**public JSON API with no authentication** when public pages are enabled:

| endpoint | gives |
| --- | --- |
| `/api/status` | `{"online": true, "timestamp": …}` — an unambiguous detection probe |
| `/api/stations` | the full station list with metadata |
| `/api/nowplaying` | live state: listeners, live presenter, now playing |

### Why it beats `/status-json.xsl`

Measured on the servers the scanner discovered:

| field | `/status-json.xsl` | AzuraCast API |
| --- | --- | --- |
| name | `server_name` | `name` + `shortcode`, `description`, `url` |
| genre | `genre` — was `"various"` on *every* host found | — |
| location | — | **`timezone`** |
| public opt-in | — | **`is_public`** |
| live presenter | — | **`live.is_live`, `live.streamer_name`** |
| listeners | `listeners` | **`listeners.current`** |
| HLS | — | **`hls_url`, `hls_enabled`, `hls_listeners`** |
| multi-station | `mounts` | `stations[]`, `mounts[]` |
| frontend/backend | `server_type` | `frontend` (icecast/shoutcast), `backend` (liquidsoap) |

Two of those matter beyond enumeration:

- **`live.is_live` and `streamer_name`** give the live-presenter-vs-automated
  distinction directly — the thing that is awkward to infer otherwise, and
  relevant to the app's mood/For-You features.
- **`hls_url` / `hls_enabled`** tells us when a station has an HLS variant,
  which feeds the stream-variant/quality feature.

### Adoption is high

Of the 47 mounts the scanner discovered in the 20-minute test, **22 (47 %) were
AzuraCast** by URL shape (`/listen/<shortcode>/`), plus 7 Radiolize and 13 plain
Icecast. So this is not a niche path — it is about half of what the scanner
finds.

### Approach

1. **Enrich, don't replace.** For every host enumerated, also try the public
   URL's `/api/status`. If it answers, fetch `/api/stations` (one extra request)
   and merge name, description, timezone, listeners, HLS and live state into the
   record.
2. **Respect `is_public: false`.** Skip those stations — the operator has not
   opted into public listing. This is a hard filter, not a preference.
3. **Detect by `/api/status`**, which is specific to AzuraCast and cheap.
4. **Enumerate fleets, not addresses.** Radiolize numbers its fleet
   (`s56.`, `s64.`, `s65.`, `my5.radiolize.com`) and AzuraCast installs are
   often at `azuracast.<domain>`. Certificate Transparency for those name
   patterns gives hosts to probe directly — no scanning required.

### Caveats

- The API is only present when the admin enabled public pages; many instances
  disable it, so absence proves nothing.
- It lives on the **web front-end (80/443)**, often behind Cloudflare, while the
  Icecast frontend is on 8000/8010. Probing the discovered IP will not reach it —
  we need the hostname from `listenurl`.
- That means the `--responders` output needs to carry `listenurl` (and ideally
  bitrate) for this enrichment to be possible downstream.

## RadioDNS — the UK broadcaster tier

**Status: decision taken to use the SI.xml.** The mechanism and the licensing
position are recorded here so the choice is deliberate rather than drifting.

### The mechanism

RadioDNS maps a *broadcast* identity to a broadcaster's own metadata server:

```
FM:  <freq×100kHz>.<pi>.<gcc>.fm.radiodns.org   →  CNAME →  authoritative FQDN
     10620.c460.ce1.fm.radiodns.org              →  rdns.musicradio.com
SRV: _radioepg._tcp.<fqdn>                       →  host:port
SI:  http://<host>:<port>/radiodns/spi/3.1/SI.xml
```

`gcc` is not a free dimension: it is the PI code's first nibble plus the
country's ECC, so a country's FM space is `205 frequencies × PI values` — about
839,680 names for the UK. **The space does not need sweeping**: `radiodns.uk`
publishes a directory of all 511 UK services across 31 authorities, derived from
Ofcom's technical parameters plus the SPI files.

Verified authorities:

| authority FQDN | broadcaster | services | SI server |
| --- | --- | --- | --- |
| `rdns.musicradio.com` | Global | 150 | `epg.musicradio.com` |
| `planetradio.co.uk` | Bauer Radio | 142 | `listenapi.planetradio.co.uk` |
| `radiodns.api.bbci.co.uk` | BBC | 70 | `radiospi.api.bbci.co.uk` |
| `rdns.aiir.net` | Aiir (community hosting) | 116 | — |
| `wireless.radiodns.metadata.radio` | News Broadcasting | 10 | — |

### The yield

Three SI fetches (1.04 MB total):

| | |
| --- | --- |
| stations | **362** |
| with a logo | **362 (100 %)** — up to 800 px, 2,512 `<multimedia>` entries |
| with a genre | 216 (60 %) |
| with a description | 362 (100 %) |
| with **more than one stream** | **142 (39 %)** |

Bearer entries map directly onto our variant model, and carry more than we do
today — bitrate, codec and a broadcaster-set preference:

```
48kbps  cost=70  audio/aacp  https://media-ssl.musicradio.com/CapitalBirmingham
128kbps cost=73  audio/mpeg  https://media-ssl.musicradio.com/CapitalBirminghamMP3
```

`cost` is the broadcaster's own quality preference, which is exactly the
"relative stream rates" signal. Genre vocabulary is controlled (18 values from
Global) rather than free-form. Each service also carries a stable
`serviceIdentifier` (`capitalbirmingham`), which solves the cross-run identity
problem for this tier.

### Licensing, and the decision

The **Radio Device Metadata Terms of Use v1.00** (`radiodns.org/terms/metadata/1.0/`)
are written for device manufacturers. Notably it states that copyright and
trademark in the metadata remain with the broadcaster, that no rights in logos
are transferred, that metadata URIs **must not be exposed to the end user**, and
that metadata must not be stored beyond the stipulated caching time or
aggregated.

**Decision: use the SI.xml anyway.** The DNS layer is public, the app is free
and non-commercial, naming the station it plays is nominative rather than
redistributive, and the alternative sources for this tier are worse. This is a
recorded judgement call by the maintainer, not an oversight — the restriction is
a contract with the broadcaster, so any objection would come from them rather
than from RadioDNS.

Two consequences to handle deliberately:

1. **Attribute.** Where this data is used, credit RadioDNS and the broadcaster.
2. **Do not sell.** Non-commercial use is part of the basis for the decision.

### Coverage

UK-centric, because RadioDNS was driven by the BBC, Global and Bauer — and the
UK is also the only country with a ready-made directory (`radiodns.uk`) and
logo packs. RadioDNS covers 26 countries, but per-broadcaster discovery has to
be done per country via `radiodns.org/coverage/`.

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

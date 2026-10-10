# South America providers

Working page for national/public broadcaster providers in South America,
mirroring the country checklist in `docs/coverage.md`. Unlike `coverage.md`
this file is **hand-maintained** — it tracks the candidate broadcaster and its
discovery URL per country until each one becomes a provider in
`src/providers/`.

Status values:

- `none` — no candidate identified yet
- `lead` — broadcaster identified, discovery URL not yet confirmed
- `url-confirmed` — discovery URL supplied and reachable
- `provider` — implemented in `src/providers/` (update `docs/coverage.md`
  by rebuilding and running `scripts/coverage.py`)

Leave the **Discovery URL** line blank and paste the URL in its place; one URL
per country (an API root, a channel-list endpoint, or a known stream pattern is
all that is needed to start).

---

## Argentina (AR) — `provider`

- **Broadcaster:** Radio y Televisión Argentina (RTA) — Radio Nacional, a
  network of ~57 LRA stations
- **Website:** <https://www.radionacional.com.ar>
- **Discovery URL:** none available. RTA publishes no station-list endpoint:
  its WordPress REST API is disabled (`401 rest_login_required`), the Icecast
  host's `status-json.xsl` is unreachable and `status.xsl` needs auth, and the
  `/reproductor/` player is not externally reachable. Catalogued as a static
  provider instead (`src/providers/radio_nacional.rs`,
  `docs/providers/radio-nacional.md`).
- **Streams:** per-station Icecast mounts on
  `https://sa.mp3.icecast.magma.edge-access.net/sc_radN`, mapped by probing the
  host and cross-checking each mount's `icy-name` against RTA's published
  emisora list. 52 live mounts catalogued; national services carry their own
  player logos, regional stations share the Radio Nacional mark.

## Bolivia (BO) — `none`

- **Broadcaster:** Red Patria Nueva (state broadcaster)
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Brazil (BR) — `provider`

- **Broadcaster:** Empresa Brasil de Comunicação (EBC) — Rádio Nacional
  (Brasília, Rio, São Paulo, São Luís, Amazônia, Alto Solimões, …) and Rádio
  MEC / MEC FM
- **Website:** <https://radionacional.ebc.com.br>, <https://radiomec.ebc.com.br>
- **Discovery URL:** `https://{site}/++api++/@search?portal_type=Emissora&b_size=100&metadata_fields=stream_url`
  on each EBC site. EBC's sites are Plone; every station is an `Emissora`
  content object whose REST representation carries the HLS `stream_url` and an
  image scale for artwork. The provider discovers the list from this API.
  Implemented in `src/providers/ebc.rs`, documented in
  `docs/providers/ebc.md`. 9 stations across the two network sites.

## Chile (CL) — `none`

- **Broadcaster:** Radio Nacional de Chile
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Colombia (CO) — `provider`

- **Broadcaster:** RTVC Sistema de Medios Públicos — Radio Nacional de Colombia,
  Radiónica, Exploremos
- **Website:** <https://www.rtvc.gov.co>, <https://www.radionacional.co>
- **Discovery URL:** `https://parrilla.rtvc.gov.co/jsonapi/node/channel?page%5Blimit%5D=50&include=field_logo_json`
  — RTVC's Parrilla Drupal JSON:API. Each channel node carries
  `field_streaming_hls`; radio streams are the ones under `Radio_`. Implemented
  in `src/providers/rtvc.rs`, documented in `docs/providers/rtvc.md`.
  3 radio stations (Radio Nacional de Colombia, Radiónica, Exploremos).

## Ecuador (EC) — `provider`

- **Broadcaster:** Pública FM (COMEP / Medios Públicos), successor to Radio
  Pública del Ecuador
- **Website:** <https://www.publicafm.ec>
- **Discovery URL:** `https://www.publicafm.ec/` — the site config carries
  `"streamingUrl"` (`https://comep.radioca.st/stream`). Implemented in
  `src/providers/publica_fm.rs`, documented in `docs/providers/publica-fm.md`.

## Guyana (GY) — `provider`

- **Broadcaster:** National Communications Network (NCN) — Voice of Guyana,
  98.1 Hot FM, Vybz 100.1 FM, plus regional stations
- **Website:** <https://ncnguyana.com>
- **Discovery URL:** `https://ncnguyana.com/radio.php` — lists each station
  with logo, name and a `listen.php?station=<CODE>` link whose page embeds the
  stream. Implemented in `src/providers/ncn.rs`, documented in
  `docs/providers/ncn.md`. 11 stations.

## Paraguay (PY) — `none`

- **Broadcaster:** Radio Nacional del Paraguay / Secretaría de Información
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Peru (PE) — `provider`

- **Broadcaster:** Radio Nacional del Perú (IRTP)
- **Website:** <https://www.radionacional.gob.pe>
- **Discovery URL:** none. IRTP's Drupal JSON:API returns 403, the site has no
  WordPress REST API, and the player resolves the stream client-side, so the
  single station is catalogued (`src/providers/irtp.rs`,
  `docs/providers/irtp.md`). The stream is a hashed HLS manifest on IRTP's
  `iblups` CDN, HTTP-only — fragile if IRTP reissues it.

## Suriname (SR) — `provider`

- **Broadcaster:** Stichting Radio-omroep Suriname (SRS)
- **Website:** <https://radiosrs.sr> (Cloudflare-blocked from the build network)
- **Discovery URL:** none. SRS's own site is bot-walled; its stream — which
  identifies itself as SRS in its ICY headers — is served by its host, SuriLive
  (`https://surilive.com:8060/;`), so the single service is catalogued
  (`src/providers/srs.rs`, `docs/providers/srs.md`).

## Uruguay (UY) — `provider`

- **Broadcaster:** Radiodifusión Nacional del Uruguay (RNU) — Radio Uruguay,
  Radio Clásica, Radio Cultura, Radio Babel
- **Website:** <https://mediospublicos.uy/category/radio/>
- **Discovery URL:** `https://mediospublicos.uy/wp-json/wp/v2/pages?search=iwstreaming&per_page=100&_fields=slug,title,content`
  — RNU's WordPress REST API. Each station has a "… en Vivo" page embedding an
  iwstreaming player widget whose id maps to `https://radios.iwstreaming.uy/<id>/stream`.
  Implemented in `src/providers/rnu.rs`, documented in `docs/providers/rnu.md`.
  4 stations.

## Venezuela (VE) — `provider`

- **Broadcaster:** Radio Nacional de Venezuela (RNV) — Informativa, Juvenil
- **Website:** <https://rnv.gob.ve>
- **Discovery URL:** none. RNV's WordPress site hard-codes the stream URLs into
  page content (`guri.tepuyserver.net/8048` Informativa, `/8156` Juvenil) and
  the host exposes no listing, so the two channels with a broadcaster-published
  stream are catalogued (`src/providers/rnv.rs`, `docs/providers/rnv.md`).
  RNV Musical and RNV Activa have no published stream URL.

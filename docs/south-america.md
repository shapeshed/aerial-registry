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

## Ecuador (EC) — `none`

- **Broadcaster:** Radio Pública del Ecuador (RPE) — verify still active
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Guyana (GY) — `none`

- **Broadcaster:** National Communications Network (NCN)
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Paraguay (PY) — `none`

- **Broadcaster:** Radio Nacional del Paraguay / Secretaría de Información
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Peru (PE) — `lead`

- **Broadcaster:** Radio Nacional del Perú (IRTP)
- **Website:** <https://www.radionacional.gob.pe>
- **Discovery URL:**
- **Verified lead:** HLS on `cdnhd.iblups.com`, e.g.
  `https://cdnhd.iblups.com/hls/0773874174fd4eba8bb9eff741d190dc.m3u8`
  (per-stream hashed manifest — needs a channel list to enumerate reliably).

## Suriname (SR) — `none`

- **Broadcaster:** Suriname National Radio (SRS)
- **Website:**
- **Discovery URL:**
- **Verified lead:**

## Uruguay (UY) — `lead`

- **Broadcaster:** Radiodifusión Nacional del Uruguay (RNU)
- **Website:** <https://rnu.gub.uy>
- **Discovery URL:**
- **Verified lead:** Radio Browser lists `CX 30 Radio Nacional` on
  `https://a1.asurahosting.com:8650/radio.mp3` and `https://stream.rcast.net/73479`
  (both unverified — likely third-party relays, not broadcaster-direct).

## Venezuela (VE) — `lead`

- **Broadcaster:** Radio Nacional de Venezuela (RNV)
- **Website:**
- **Discovery URL:**
- **Verified lead:** Icecast/Shoutcast mounts on `guri.tepuyserver.net`, e.g.
  `https://guri.tepuyserver.net/8048/stream` (Informativa) and
  `https://guri.tepuyserver.net/8156/stream` (Juvenil), both live
  (`200 audio/mpeg`). Enumerate the full mount set to cover the music services.

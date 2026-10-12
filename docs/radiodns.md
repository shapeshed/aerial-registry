# RadioDNS

RadioDNS ("hybrid radio") maps a **broadcast** radio identity to internet
services published by the broadcaster. For this project it is the source of the
high-quality broadcaster tier: station names, artwork, and stream variants with
bitrate and codec — the things our scanner cannot produce.

Two ETSI standards define it:

- **TS 103 270** — hybrid lookup (how to build the DNS query)
- **TS 102 818** — Service and Programme Information ("SPI"), the metadata format

Both are free to download and use; RadioDNS charges no licence or patent fees.

## The mechanism

### 1. Build an FQDN from broadcast parameters

A station's broadcast identity is turned into a DNS name under `radiodns.org`.
`gcc` is a **Global Country Code**, built as the PI code's first nibble plus the
country's ECC:

```
gcc = PI[0] + ECC          e.g. UK: "C" + "E1" = ce1
```

| bearer | FQDN | parameters |
| --- | --- | --- |
| FM (RDS) | `<freq 5char>.<pi 4hex>.<gcc>.fm.radiodns.org` | `10620.c460.ce1.fm.radiodns.org` |
| DAB | `<scids>.<sid>.<eid>.<gcc>.dab.radiodns.org` | `0.d313.10a5.de0.dab.radiodns.org` |
| DRM | `<sid 6hex>.drm.radiodns.org` | |
| AMSS | `<sid>.amss.radiodns.org` | |
| IBOC (HD Radio, US) | `<tx 5hex>.<cc>.hd.radiodns.org` | `cc` = 0x292 for the USA; `tx` = FCC facility code |

FM frequency is in units of 100 kHz as 5 characters: 95.8 MHz → `09580`.

The country table (ECC + PI nibble per country) is available from
`radiodns/java-CountryCodeResolver` (`countries.csv`) and
`radioplayer/radioplayer-automotive-country-code-resolver`.

### 2. Resolve to the broadcaster's authoritative FQDN

```
10620.c460.ce1.fm.radiodns.org   CNAME   rdns.musicradio.com
```

The target is the broadcaster's own domain, and **it has no naming pattern** —
`rdns.musicradio.com`, `planetradio.co.uk`, `radiodns.api.bbci.co.uk`,
`playsr.com`, `rdns.khz.se`, `radiodns.emfeng.com`. It must be discovered,
not guessed from the brand.

### 3. Find the metadata server

```
_radioepg._tcp.<authoritative fqdn>   SRV   0 100 80 epg.musicradio.com.
```

`_radiospi._tcp` is the newer application name; `_radioepg._tcp` is what the
deployed broadcasters use. `radioepg`, `radiospi`, `radiotag`, `radiovis` are
the four known applications.

### 4. Fetch the SI

```
http://<srv target>:<port>/radiodns/spi/3.1/SI.xml
```

Usually plain **HTTP on port 80** — the SI servers have no TLS, which matters
(see "what does not work").

### What the SI contains

One document per broadcaster, covering **all** their stations. Per `<service>`:

| element | gives |
| --- | --- |
| `shortName` / `mediumName` / `longName` | station names |
| `shortDescription` / `longDescription` | descriptions |
| `genre` | genre, from a **controlled vocabulary** (TV Anytime URNs) |
| `multimedia` | **logos at 5–17 resolutions** |
| `bearer` | broadcast identities **and IP streams**, with `bitrate`, `mimeValue` (codec) and `cost` (a broadcaster-set preference) |
| `radiodns serviceIdentifier` | a **stable per-station identifier** |
| `link` | homepage and other URIs |

The `cost` attribute is exactly the "relative stream rate" ranking signal, and
it is published by the broadcaster rather than inferred.

## Discovery: finding the authorities

The authoritative FQDN is the only thing that cannot be derived, so each
broadcaster must be bootstrapped from **one real broadcast identity**.
Ranked by cost:

### 1. Domain probing (cheapest)

Try `rdns.<broadcaster>`, `radiodns.<broadcaster>`, `radio.<cc>` and check for
`_radioepg._tcp` SRV. One DNS query per guess.

Worked for: Global (`rdns.musicradio.com`), NRK (`radiodns.nrk.no`), Norway's
local radio (`rdns.radio.no`).
Failed for: Sweden (0/8), Ireland (0/15) — brand names don't predict domains.

### 2. Regulator identity data (best where it exists)

| country | source | status |
| --- | --- | --- |
| **UK** | Ofcom transmitter parameters spreadsheet | ✅ verified |
| **USA** | `picodes.nrscstandards.org` — 9,096 FM stations with call sign, PI code, frequency, facility ID, regenerated **daily from the FCC LMS** | ✅ verified |
| Germany | ARD/VAUnet publish; BNetzA for frequencies | unverified |
| France | Arcom frequency assignments | unverified |
| Australia | ACMA Radiocommunications Licence Register | unverified |
| Canada | CRTC / Radioplayer Canada | unverified |

The US table exists *because* of RadioDNS: the NRSC states its translator PI
codes were assigned so that "every carrier frequency/PI code combination [is]
unique", as RadioDNS requires.

US RBDS PI codes are also **computable from the call sign** (NRSC-4-B D.7.1),
so the US is derivable even without the published table:

```
KGTB → (G-65)*676 + (T-65)*26 + (B-65) + 4096                = 0x21C7
WKTI → value + 21672                                         = 0x7106
exceptions: _0__ → A___ before use;  __00 → AF__
```

### 3. Frequency-narrowed DNS sweep

Where no identity source exists, sweep only the frequencies actually in use
(from Wikipedia, a regulator, or a transmitter database):

```
<freq>.<pi>.<gcc>.fm.radiodns.org   for all 4096 PI values of the country nibble
```

Cost is *10 frequencies × 4096 = 41k names ≈ 3 min*, versus *205 × 4096 = 840k*.
The Sweden run found 3 authorities from 10 frequencies at a **0.58 %** hit
density, against 0.127 % for the UK's full band — narrowing by real frequencies
raises the yield ~4.5×.

### 4. Full-band DNS sweep (completeness proof)

205 frequencies × 4096 PI codes = **839,680 names**, **~58 min** at ~240 names/s.
This is the only way to *prove* a country complete without a published list.
DAB cannot be swept (16 × 65,536 × 65,536 ≈ 7×10¹⁰).

### Published directories

- **`radiodns.uk`** — all 511 UK services across **31 authorities**, derived from
  Ofcom data plus the SI files. Also publishes logo packs and three mismatch
  reports. Source: `njh/radiodns-uk` (MIT).
- **`radiodns.org/coverage/`** — per-country participating broadcasters with
  station names and share of listening. Not machine-readable, but it tells you
  *who* to look for.

## Operational notes (learned the hard way)

- **Never query `ns1.radiodns.org` directly at volume.** It answered ~20 % with
  39 % timeouts; the same query via a public resolver answered **96/96 in 0.3 s**.
  The throttling is self-inflicted. Sweeps should go through a caching resolver.
- **A sampled sweep proves nothing.** Iceland has ~3 transmitters in an
  839,680-name space, so a 40,000-name sample has **0.14 expected hits**. Size a
  sweep from the expected station count or don't run it.
- **Frequency narrowing needs a *complete* frequency list.** Rás 1 is a national
  network with dozens of transmitters; the two Wikipedia listed weren't enough.
- **The zone is closed.** AXFR is REFUSED on `radiodns.org` and `fm.radiodns.org`,
  and it is not DNSSEC-signed, so no NSEC walking. There is no bulk enumeration.
- **Some registrations are dangling.** `c752.radioplayer` (SE) and
  `c826.radioplayer` (UK) are CNAMEs to names with **no TLD** — unresolvable, no
  SRV, no SI. `radiodns.uk` reports *"No Service Information"* for them.

## Licensing, and the decision

The **Radio Device Metadata Terms of Use v1.00**
(`radiodns.org/terms/metadata/1.0/`) are written for device manufacturers. They
state that copyright and trademark in the metadata remain with the broadcaster,
that no rights in logos are transferred, that metadata URIs **must not be
exposed to the end user**, and that metadata must not be stored beyond the
stipulated caching time or aggregated.

**Decision: use the SI.xml anyway.** The DNS layer is public, the app is free and
non-commercial, naming the station it plays is nominative rather than
redistributive, and the alternatives for this tier are worse. This is a recorded
judgement call by the maintainer, not an oversight — the restriction is a
contract with the *broadcaster* (not RadioDNS), so any objection would come from
them.

Two consequences to handle deliberately:

1. **Attribute** — credit RadioDNS and the broadcaster where the data is used.
2. **Do not sell** — non-commercial use is part of the basis for the decision.

## Verified results

| country | authority | SI server | services | logos | streams |
| --- | --- | --- | --- | --- | --- |
| **UK** | `rdns.musicradio.com` (Global) | `epg.musicradio.com` | 150 | 950 | 1,161 |
| | `planetradio.co.uk` (Bauer) | `listenapi.planetradio.co.uk` | 142 | 710 | 547 |
| | `radiodns.api.bbci.co.uk` (BBC) | `radiospi.api.bbci.co.uk` | 70 | 852 | 475 |
| **Sweden** | `playsr.com` (Sveriges Radio) | `spi.playsr.com` | 38 | 195 | 222 |
| | `rdns.khz.se` (RIX, STAR, BANDIT…) | `rdns.khz.se` | 14 | 70 | 42 |
| **Norway** | `rdns.radio.no` (local) | `repg.rdns.radio.no` | 79 | 1,343 | **0** |
| | `radiodns.nrk.no` (NRK) | `static.nrk.no` | 50 | 255 | **0** |
| **USA** | `radiodns.emfeng.com` (EMF) | `radiodns.emfeng.com` | 15 | 96 | 1,240 (HLS) |
| **Iceland** | — | — | **0 (proven by full FM sweep)** | | |

Three market shapes, and they differ in what the SI is good for:

| market | SI yields |
| --- | --- |
| broadcast **and** streaming (UK, Sweden) | names, logos, **IP stream variants with bitrate + cost** |
| **DAB-only** (Norway, FM switched off 2017) | names, logos **only** — the audio is already broadcast |
| US | names, logos, **HLS** streams (ABR-native, so no rate variants to index) |

## What RadioDNS does not cover

- **Radioplayer** (the independents tier: *"176 UK Radio Stations"*). Its UK and
  Swedish registrations are dangling, its `api.` hosts from CT do not resolve,
  and it publishes no client or docs. Partner-gated. Ireland pending.
- **DAB in closed markets** — unsweepable; needs ensemble/service IDs.
- **HLS in DAB-style metadata-only markets** — no rates to index.
- **Self-hosted Icecast/Shoutcast** — the scanner's tier, which RadioDNS does
  not see at all.

## Tooling

Reusable pieces found:

| item | what it gives |
| --- | --- |
| `radiodns/pyradiodns`, `java-radiodns`, `radiodns-cli`, `radiodns-sdk-js` | lookup implementations |
| `radiodns/java-CountryCodeResolver` | country → ECC + PI nibble table |
| `njh/radiodns-uk` (MIT) | a complete crawl: Ofcom → resolve → fetch SI → logopacks, plus the TV Anytime genre map |
| `ebu/radiodns-manager` | the EBU's own management platform |
| `magicbadger/python-hybridspi`, `radiodns/spi-schema` | SPI parsing |
| `radiodns.org/terms/metadata/1.0/` | the licensing position |

Local scratch tooling used for the investigation lives in `/tmp/opencode/`:
`radiodns_brute.py` (the DNS sweeper), `countries.csv` (the ECC/nibble table),
and the fetched `si_*.xml` corpus.

## Provider design

```
per-country loader (regulator / domain probe / sweep)
        ↓  broadcast identities: fm/pi/freq, dab/eid/sid/scids, hd/tx
   resolve()        → authoritative FQDN      (DNS via resolver, retry)
   applications()   → SI endpoint             (SRV _radioepg._tcp)
   fetch + parse    → Station + logos + streams[] variants
```

One trait implementation per country for the loader; everything else is
country-agnostic. The loader is the only bespoke part, and for the US it is
already published by the NRSC.

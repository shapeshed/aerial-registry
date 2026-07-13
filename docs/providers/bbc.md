# BBC Provider

The BBC exposes a public REST API called RMS (Radio Metadata Service) that
covers all BBC radio networks. No authentication is required. The full API
reference is at <https://1fpvlc47jd.apidog.io/>.

## Station Discovery

### Networks endpoint

API docs: <https://1fpvlc47jd.apidog.io/> — see the Networks section.

Fetch all BBC networks to enumerate available stations:

```
GET https://rms.api.bbc.co.uk/v2/networks?limit=100
```

The response is a JSON object with a `data` array. Each element represents one
network. The fields relevant to the registry are:

| Field                | Type   | Notes                                                                                  |
| -------------------- | ------ | -------------------------------------------------------------------------------------- |
| `id`                 | string | The network's own id. Use this for the logo URL (see below) — it usually equals `default_service_id`, but not always. |
| `default_service_id` | string | Stable BBC service identifier, e.g. `bbc_radio_one`. Used in all subsequent API calls. |
| `title`              | string | Human-readable station name. Fall back to `titles.primary` if absent.                  |

If neither `title` nor `titles.primary` is present, derive a name from the
service ID: strip the `bbc_` prefix, replace underscores with spaces, and apply
the capitalisation rules in the table below.

**Service ID capitalisation rules:**

| Token    | Display |
| -------- | ------- |
| `bbc`    | BBC     |
| `fm`     | FM      |
| `mw`     | MW      |
| `lw`     | LW      |
| `am`     | AM      |
| `1xtra`  | 1Xtra   |
| `6music` | 6 Music |
| `wm`     | WM      |

All other tokens are title-cased. Prepend "BBC" if the result does not already
start with it.

### Stream URL resolution

The BBC does not return a direct stream URL from the networks endpoint. BBC
streams come in two variants that must be fetched and stored separately:

| Variant       | Manifest URL pattern                                                                                                                 |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| National (UK) | `http://a.files.bbci.co.uk/ms6/live/3441A116-B12E-4D2F-ACA8-C1984642FA4B/audio/simulcast/hls/uk/pc_hd_abr_v2/ak/{serviceId}.m3u8`    |
| International | `http://a.files.bbci.co.uk/ms6/live/3441A116-B12E-4D2F-ACA8-C1984642FA4B/audio/simulcast/hls/nonuk/pc_hd_abr_v2/ak/{serviceId}.m3u8` |

The manifest UUID `3441A116-B12E-4D2F-ACA8-C1984642FA4B` is a stable BBC
infrastructure identifier. Parse each response as plain text (M3U8 format) and
take the first line that begins with `http` — that is the resolved variant
stream URL.

**Generating registry entries:** produce two entries per BBC network, one for
each variant stream. Name them as follows:

- National: use the station name as-is, e.g. `BBC Radio 1`
- International: append `(International)`, e.g. `BBC Radio 1 (International)`

For UK listeners the national stream is preferred and should be treated as the
primary entry. Skip a variant if its manifest returns no resolvable stream URL.

### Logo URL

BBC logos are available as SVGs using a predictable URL pattern:

```
https://sounds.files.bbci.co.uk/3.9.4/networks/{id}/colour_default.svg
```

No additional request is needed — construct this URL directly from the
network's `id` field, **not** `default_service_id`. They're equal for most
networks, but diverge for a few: Radio 4 (`bbc_radio_four` vs
`bbc_radio_fourfm`), Radio Scotland (`bbc_radio_scotland` vs
`bbc_radio_scotland_fm`), and Radio Wales (`bbc_radio_wales` vs
`bbc_radio_wales_fm`). Using `default_service_id` for the logo 404s for those
three.

## Data Points

| Field        | Source                | Notes                                      |
| ------------ | ---------------------- | ------------------------------------------ |
| `name`       | Networks `title`       | Fall back to derived name if absent        |
| `stream_url` | HLS manifest           | First HTTP line from the M3U8 variant list |
| `logo_url`   | Constructed from `id`  | SVG format                                 |
| `country`    | Hardcoded                  | Always `United Kingdom` / `GB`             |

Tags and description are not available from this provider.

## API Behaviour Notes

- **No authentication required.** All RMS endpoints are public.
- **User-Agent header.** Include a descriptive `User-Agent` header on all
  requests to avoid being treated as an unidentified scraper.
- **`limit=100` on networks.** The BBC currently has fewer than 100 networks but
  the parameter should be specified to avoid a server-side default that may be
  lower.

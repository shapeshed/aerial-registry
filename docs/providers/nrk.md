# NRK Provider

NRK (Norsk rikskringkasting) is the Norwegian public broadcaster. Its public
playback API — the same one used by NRK's own radio website and apps — serves
per-channel metadata and stream manifests unauthenticated. There is no
authentication or reverse-engineered signing involved.

API docs: <https://psapi.nrk.no/documentation/>

## Station Discovery

NRK's public API has no endpoint that enumerates all radio channels (the one
GitHub project found using `/radio/linear/channels` requires a bearer token
and is not part of the documented public API — this provider avoids it
entirely). Instead, known channel IDs are probed individually against two
endpoints.

### Metadata endpoint

```
GET https://psapi.nrk.no/playback/metadata/channel/{channelId}
```

| Field                        | Notes                                                  |
| ---------------------------- | -------------------------------------------------------- |
| `preplay.titles.title`        | Station name, e.g. `NRK P1`                              |
| `preplay.description`         | Usually empty; drop if so                                |
| `preplay.poster.images[]`     | Several fixed-width thumbnails (not square); pick whichever `pixelWidth` is closest to 600 |

If `playability` is `nonPlayable` (e.g. geo-blocked), there will be no usable
stream — check the manifest response rather than this field, since a
channel can still return metadata while being unplayable.

### Manifest endpoint

```
GET https://psapi.nrk.no/playback/manifest/channel/{channelId}
```

`playable.assets[0].url` is the direct HLS stream URL. **The stream's internal
slug does not always match the channel ID** — regional P1 district channels
in particular resolve to an internal `p1_dkNN` identifier (e.g. `p1_ostfold`
resolves to `p1_dk15`). Always take the URL from the manifest response; never
construct it from the channel ID.

### Channel ID list

Confirmed working channel IDs, assembled by probing the API (national/
thematic channels, plus every P1 district edition):

`p1`, `p2`, `p3`, `mp3`, `klassisk`, `jazz`, `radio_super`, `alltid_nyheter`,
`folkemusikk`, `sport`, `p1pluss`, `sapmi`, and the P1 district IDs:
`p1_buskerud`, `p1_finnmark`, `p1_hordaland`, `p1_innlandet`,
`p1_more_romsdal`, `p1_nordland`, `p1_oslo_akershus`, `p1_ostfold`,
`p1_rogaland`, `p1_sogn_fjordane`, `p1_sorlandet`, `p1_telemark`, `p1_troms`,
`p1_trondelag`, `p1_vestfold`.

**`nrksuper` is deliberately excluded** — it's NRK's TV channel ID, not radio,
and resolves to `playability: nonPlayable` / `isGeoBlocked: true`.

This list is maintained as a hardcoded const in the provider. If NRK adds a
new radio channel it will need a new ID added here manually.

## Data Points

| Field          | Source                             | Notes                                             |
| -------------- | ------------------------------------ | -------------------------------------------------- |
| `name`         | `preplay.titles.title`                |                                                    |
| `stream_url`   | `playable.assets[0].url` (manifest)   | Direct HLS, no auth, no expiring token             |
| `logo_url`     | `preplay.poster.images[]` (metadata)  | Not square; pick closest to 600px wide             |
| `country`      | Hardcoded                             | Always `Norway` / `NO`                             |
| `description`  | `preplay.description`                 | Usually empty                                      |
| `provider_id`  | Channel ID                            |                                                    |
| `trusted`      | Hardcoded                             | `true` — broadcaster-direct, skip liveness checks |

Tags are not available from this provider.

## API Behaviour Notes

- **No authentication required** for metadata or manifest lookups.
- **Two requests per channel**, fetched concurrently (`tokio::join!` per
  channel, all channels fanned out via `join_all`) — metadata for
  name/description/logo, manifest for the stream URL.
- **Check `playable` in the manifest, not `playability` in the metadata.** A
  channel ID can return valid metadata while being non-playable (e.g. the TV
  channel ID `nrksuper`).

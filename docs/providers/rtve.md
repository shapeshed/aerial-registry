# RTVE Provider

RTVE publishes public HLS streams for Radio Nacional de España and its regional
RNE/Radio 5 variants. No authentication is required. The provider uses stable
RTVE stream URL patterns and documented station pages rather than scraping HTML.

Reference: <https://ulisesgascon.github.io/RTVE-API/>

## Station Discovery

### National stations

The national RNE services have stable direct HLS URLs:

| Station          | Stream URL                                                                    |
| ---------------- | ----------------------------------------------------------------------------- |
| Radio Nacional   | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_r1_main.m3u8`                 |
| Radio Clásica    | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_r2_main.m3u8`                 |
| Radio 3          | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_r3_main.m3u8`                 |
| Ràdio 4          | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_r4_main.m3u8`                 |
| Radio 5          | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_r5_madrid_main.m3u8`          |
| Radio Exterior   | `https://rtvelivestream.rtve.es/rtvesec/rne/rne_re_main.m3u8`                 |

### Regional stations

Regional Radio Nacional and Radio 5 streams follow this URL pattern:

```
GET https://rnelivestream.rtve.es/{channel}/{regionCode}/128/seglist.m3u8
```

`channel` is `rne1` or `rne5`. `regionCode` is the RTVE regional stream code,
for example `mad`, `cat`, `and`, or `vlc`. The provider maintains the known
regional code list and emits one station per stream.

## Data Points

| Field          | Source                         | Notes                               |
| -------------- | ------------------------------ | ----------------------------------- |
| `name`         | Hardcoded station metadata     | Regional names include the region   |
| `stream_url`   | RTVE HLS URL pattern           | Direct unauthenticated HLS streams  |
| `logo_url`     | Public broadcaster logo URLs   | One logo per RNE service            |
| `country`      | Hardcoded                      | Always `Spain`                      |
| `country_code` | Hardcoded                      | Always `ES`                         |
| `tags`         | Hardcoded station metadata     | Includes public radio/news/regional |
| `description`  | Hardcoded station metadata     | Short service description           |

## API Behaviour Notes

- **No authentication required.** The RTVE streams are public.
- **No HTML scraping.** Regional metadata comes from maintained RTVE stream
  codes and public station page references.
- **Trusted provider.** RTVE records are broadcaster-direct and are marked as
  trusted by the implementation.

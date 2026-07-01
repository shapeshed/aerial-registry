# Curated Provider

The curated provider is a static, hand-maintained list of independent stations
that don't have their own broadcaster-direct provider (community radio,
independent electronic/music stations, etc.). It reads entirely from a
checked-in file — there is no network discovery step.

Source file: `stations.toml`

## Station Discovery

### stations.toml format

```toml
[[stations]]
name = "NTS Radio 1"
country_code = "GB"
stream_url = "https://stream-relay-geo.ntslive.net/stream"
logo_url = "https://media.nts.live/misc/nts_logo_1000x1000.png"
tags = ["indie", "experimental", "electronic"]
```

| Field          | Required | Notes                                  |
| -------------- | -------- | --------------------------------------- |
| `name`         | yes      |                                          |
| `country_code` | yes      | ISO 3166-1 alpha-2                      |
| `stream_url`   | yes      |                                          |
| `logo_url`     | no       |                                          |
| `tags`         | no       | Must match the allowed tag list         |
| `description`  | no       |                                          |

New entries come from two sources:

- Hand-added by a maintainer directly editing `stations.toml`.
- Proposed by the discovery agent (`scripts/discover.py`, run weekly by the
  `discover.yml` workflow) via pull request, sourced from Radio Browser vote
  counts and cleaned up by an AI assessment pass before the PR is opened.

All entries — whichever source — are reviewed on the PR before merge.

## Data Points

| Field          | Source            | Notes                                       |
| -------------- | ------------------ | -------------------------------------------- |
| `name`         | `stations.toml`   |                                              |
| `stream_url`   | `stations.toml`   |                                              |
| `logo_url`     | `stations.toml`   | Optional                                    |
| `country_code` | `stations.toml`   |                                              |
| `tags`         | `stations.toml`   | Optional                                    |
| `description`  | `stations.toml`   | Optional                                    |
| `trusted`      | Hardcoded          | `false` — curated entries go through liveness checks like aggregator sources |

## API Behaviour Notes

- **No network discovery.** Parsing failures on `stations.toml` log a warning
  and return an empty list rather than aborting the pipeline.
- **Not trusted.** Unlike broadcaster-direct providers, curated stations are
  not exempt from liveness checks — they can go stale or move without a
  broadcaster maintaining the URL.
- **Pruning.** The nightly build runs `cargo run -- prune-curated` afterwards,
  which removes entries that failed the liveness policy and appends them to
  `stations_rejected.toml` so the discovery agent does not re-propose them.

# Registry maintenance plan

The registry is rebuilt from scratch every night from ~20 provider APIs. That
design is simple and self-healing for metadata, but it has no memory: it cannot
tell a station that has genuinely gone away from a provider API that failed for
one night, and it cannot tell a station that is offline from one that is
geo-blocked from the build machine. This plan adds enough state to answer those
questions while keeping the nightly build automated and low-touch.

## Problems this addresses

1. **Silent provider loss.** A transient provider failure (e.g. Wireless
   returning a malformed response) removes every station for that provider from
   the published registry until the next successful run. Users lose stations
   overnight for no real-world reason.
2. **No station identity over time.** Nothing records when a station first
   appeared, when it was last seen, or whether it changed name or stream URL.
   A rename looks identical to a removal plus an unrelated addition.
3. **Liveness is a single sample.** One failed probe on one night from one
   network location is treated as truth. Geo-blocked streams (HTTP 403/451
   from a GitHub-hosted runner) look identical to dead streams.
4. **Categorisation quality is uneven.** Provider tags are inconsistent;
   Radio Browser tags are noisy. Local-model enrichment works (see
   `docs/ai-evaluation-plan.md`) but is not wired into the pipeline.
5. **No visibility.** The nightly run publishes with no diff report, so
   regressions are only noticed in the app.

## Step 1 — previous-registry guard (implemented)

`src/pipeline/guard.rs` runs after liveness and before output. It compares
per-provider station counts against a previous registry snapshot, and where
a provider lost more than half of its stations it discards the partial
output and carries yesterday's entries forward, logging a warning. Carried
entries whose stream URL is now owned by another provider are skipped to
avoid re-introducing duplicates.

The app no longer fetches a hosted registry over the network — it bundles a
snapshot built from a local pipeline run and tested on a device before it
ships — so there is nothing published for the guard to pull from either.
Instead it reads a local file:

- `AERIAL_PREVIOUS_REGISTRY_PATH` points at a `registry.json` or
  `registry.json.gz` to compare against — typically a local copy of the
  app's currently-shipped `app/src/main/registry/registry.json`, i.e. the
  last human-approved state. The nightly workflow checks out `shapeshed/aerial`
  read-only to get this file; a local run points it at your own checkout.
- Unset (the common case for an ad hoc local run) disables the guard.

This is deliberately stateless on its own — the last-shipped registry is the
state — but "last-shipped" now means "the file you point it at", not a
network fetch.

## Step 2 — station state store and prune hysteresis (implemented)

`src/pipeline/state.rs` holds a small SQLite database (`state.db`, override
with `AERIAL_STATE_DB`; empty string disables it and hysteresis with it),
persisted between nightly runs at `s3://<bucket>/state/state.db`:

```sql
CREATE TABLE station_state (
  provider     TEXT NOT NULL,
  provider_id  TEXT NOT NULL,
  first_seen   TEXT NOT NULL,   -- ISO date
  last_seen    TEXT NOT NULL,
  consecutive_failures INTEGER NOT NULL DEFAULT 0,
  last_status  TEXT,            -- live | geo_blocked | unreachable | ...
  source_hash  TEXT,            -- hash of provider-supplied fields (step 4)
  PRIMARY KEY (provider, provider_id)
);
```

Keyed on `(provider, provider_id)` — the composite identity the app already
uses — falling back to the stream URL for providers with no id. Rows unseen
for 90 days are dropped.

Rules (enforced in `src/pipeline/liveness.rs`):

- **Three strikes before prune.** An untrusted station failing liveness is
  only dropped after three consecutive nightly failures. One bad night keeps
  the station with `consecutive_failures` incremented. `UnsupportedScheme` is
  a property of the URL, not the network, and still prunes immediately.
  Without a state store there is no memory, so behaviour falls back to
  immediate pruning.
- **Geo-suspect statuses never prune.** HTTP 403/451 responses from the runner
  are recorded (`last_status = geo_blocked`) but never remove a station and
  never count as failures; the build machine's location is not the
  listener's. The same rule applies to `prune-curated`.
- **Trusted providers never auto-prune.** Liveness skips them entirely.
  Opening a GitHub issue when a trusted station fails repeatedly lands with
  the diff report in step 3.
- **Renames are updates, not churn.** Same `(provider, provider_id)` with a
  changed name or stream URL keeps its row and its `first_seen`; surfacing
  the change lands with the diff report in step 3.

## Step 3 — nightly diff report and anomaly issues (implemented)

`src/pipeline/report.rs` diffs the new registry against the previous one
(keyed on `(provider, provider_id)`, so a rename shows as a rename and not as
remove-plus-add) and appends a report to `$GITHUB_STEP_SUMMARY`: totals,
per-provider previous/current/added/removed table, renames, and any guard
interventions.

When the guard intervened, the intervention table is also written to
`anomalies.md` and the nightly workflow opens a `Nightly registry anomalies`
issue (or comments on the open one). Anomalies are the only thing that pages
a human; a healthy run is quiet.

## Step 4 — Enrichment as a committed, hand-edited overlay (implemented)

- Corrections live in a committed file (`enrichment.toml`, array of
  `[[station]]` tables keyed on `(provider, provider_id)`), holding overrides
  for name, tags, and description, plus `reject = true` to drop a record.
- The nightly build applies the overlay deterministically
  (`src/pipeline/overlay.rs`, between enrich and liveness) — no model, no
  network dependency, reproducible output. Edit the file by hand; an entry
  survives until that station's source data changes (`source_hash`).
- An earlier AI-assisted version of this (a weekly job proposing overlay
  entries via an LLM) was removed to simplify the pipeline. It may return
  later as an addition on top of this same file format.

## Step 5 — Radio Browser bulk coverage + country overlays (implemented)

The ~40 broadcaster-direct providers plus `curated` only cover a fraction of
what a global radio directory needs. `src/providers/radio_browser.rs` fills
the rest: a paginated bulk fetch of the public Radio Browser catalog,
mechanically filtered (a resolved stream URL, not a `.pls`/`.m3u` playlist
link — nothing else), marked `trusted: false`.

This interacts with the other steps rather than duplicating them:

- **Dedup prefers trusted providers.** `src/pipeline/dedup.rs` drops any
  Radio Browser entry that duplicates a trusted provider's station by exact
  `(name, country_code)`, even when the stream URLs don't match (a
  broadcaster often publishes a different CDN/bitrate mirror than the one
  Radio Browser happened to index).
- **The guard (Step 1) protects the bulk fetch too.** Radio Browser's public
  API returns occasional 502s under load; if a run's fetch fails outright,
  the provider returns no stations for that run rather than erroring, and
  the guard carries the previous run's Radio Browser entries forward instead
  of publishing a near-empty catalog.
- **Corrections are human-edited, split by country.** Radio Browser data is
  community-submitted and uneven — wrong names, dead/superseded stream URLs,
  bad logos. `overlays/radio-browser/<COUNTRY>.toml` (see the directory's own
  README) holds corrections in the same `Entry` shape as `enrichment.toml`,
  merged into the same `overlay::apply()` pass, so a fix survives indefinitely
  and a PR only ever touches one country's file.
- **Tag enrichment (Step 4's neighbour, `pipeline::enrich.rs`) skips it.**
  Radio Browser stations already carry their own tags from the bulk fetch,
  so re-querying the same API by name per station would be redundant load on
  an upstream that's already prone to 502s.

## Order of work

1. ~~Previous-registry guard~~ (done)
2. ~~State store + three-strike hysteresis + geo-aware liveness policy~~ (done)
3. ~~Nightly diff summary + anomaly-only issues~~ (done)
4. ~~Enrichment as a committed, hand-edited overlay~~ (done)
5. ~~Radio Browser bulk provider + country-partitioned overlays~~ (done)

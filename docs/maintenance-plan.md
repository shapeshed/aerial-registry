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

`src/pipeline/guard.rs` runs after liveness and before output. It fetches the
previously published registry from CloudFront, compares per-provider station
counts, and where a provider lost more than half of its stations it discards
the partial output and carries yesterday's entries forward, logging a warning.
Carried entries whose stream URL is now owned by another provider are skipped
to avoid re-introducing duplicates.

- `AERIAL_PREVIOUS_REGISTRY_URL` overrides the comparison source.
- `AERIAL_PREVIOUS_REGISTRY_URL=""` disables the guard (useful for local
  experiment builds that intentionally run a subset of providers).

This is deliberately stateless: the published registry itself is the state.

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

## Step 4 — AI enrichment as a committed overlay (implemented)

Model inference is decoupled from the nightly build:

- Enrichment results live in a committed file (`enrichment.toml`, array of
  `[[station]]` tables keyed on `(provider, provider_id)`), holding overrides
  for name, tags, and description, plus `reject = true` for records the model
  identified as junk (never applied to trusted stations).
- The nightly build applies the overlay deterministically
  (`src/pipeline/overlay.rs`, between enrich and liveness) — no model, no
  network dependency, reproducible output. Hand edits to the file are legal
  and survive until that station's source data changes.
- `cargo run -- enrich-overlay` re-runs the model only for stations that are
  new or whose `source_hash` (name, country and description as supplied by
  the provider) changed, then rewrites the file sorted. The weekly
  `enrich.yml` workflow runs it with Claude Haiku via the Anthropic
  OpenAI-compatible endpoint and opens a PR with the delta; review is a quick
  scan of a small diff. Low-confidence assessments record the hash but change
  nothing, so they are not retried every week.
- The model backend is any OpenAI-compatible endpoint via `AERIAL_AI_URL` /
  `AERIAL_AI_MODEL` / `AERIAL_AI_API_KEY`: a local `llama-server` (see
  `docs/local-ai-llamacpp.md`) for development and prompt tuning, or Claude
  Haiku in CI.

Model findings from the evaluation phase: Gemma is strongest on tags and
descriptions, Llama best preserves public station titles. The response parser
tolerates fenced/wrapped/prose-embedded JSON; remaining work is prompt tuning.

## Order of work

1. ~~Previous-registry guard~~ (done)
2. ~~State store + three-strike hysteresis + geo-aware liveness policy~~ (done)
3. ~~Nightly diff summary + anomaly-only issues~~ (done)
4. ~~AI enrichment overlay + weekly delta job~~ (done)

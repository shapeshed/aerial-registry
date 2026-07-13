# Registry maintenance plan

The registry is rebuilt from scratch every night from ~20 provider APIs. That
design is simple and self-healing for metadata, but it has no memory: it cannot
tell a station that has genuinely gone away from a provider API that failed for
one night, and it cannot tell a station that is offline from one that is
geo-blocked from the build machine. This plan adds enough state to answer those
questions while keeping the nightly build automated and low-touch.

## Problems this addresses

1. ~~**Silent provider loss.**~~ No longer applicable — see the note on Step 1
   below.
2. **No station identity over time.** Nothing records when a station first
   appeared, when it was last seen, or whether it changed name or stream URL.
   A rename looks identical to a removal plus an unrelated addition.
3. **Liveness is a single sample.** One failed probe on one night from one
   network location is treated as truth. Geo-blocked streams (HTTP 403/451
   from a GitHub-hosted runner) look identical to dead streams.
4. **Categorisation quality is uneven.** Provider tags are inconsistent;
   Radio Browser tags are noisy. Local-model enrichment works (see
   `docs/ai-evaluation-plan.md`) but is not wired into the pipeline.
5. ~~**No visibility.**~~ No longer applicable — see the note on Step 3 below.

## Step 1 — previous-registry guard (removed)

The app no longer fetches a hosted registry over the network — it bundles a
snapshot built from a local pipeline run — so there is no continuously-served
"previous registry" left to protect or compare against. `src/pipeline/guard.rs`
(which fetched the last published `registry.json.gz` from CloudFront and
carried a provider's previous entries forward if it lost more than half its
stations in one run) has been removed, along with the nightly workflow's
upload of `registry.json.gz` to S3 and its CloudFront invalidation step.

Silent provider loss (problem 1) is consequently unmitigated at the pipeline
level again: a provider API failing for one run does drop its stations from
that run's output with no automatic recovery. This is an accepted tradeoff —
each run's `registry.json.gz` is reviewed by a human before it's copied into
the app, rather than published automatically for something else to consume.

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
- **Trusted providers never auto-prune.** Liveness skips them entirely. A
  trusted station failing repeatedly has no automatic surfacing now that
  Step 3 is removed (see below) — check `state.db` or the run's own logs.
- **Renames are updates, not churn.** Same `(provider, provider_id)` with a
  changed name or stream URL keeps its row and its `first_seen`.

## Step 3 — nightly diff report and anomaly issues (removed)

`src/pipeline/report.rs` diffed the new registry against the previously
published one and appended a summary to `$GITHUB_STEP_SUMMARY`, opening a
`Nightly registry anomalies` issue when the guard (Step 1) had intervened.
Removed alongside Step 1, since it had no data source once nothing published
a "previous registry" to diff against.

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

1. ~~Previous-registry guard~~ (removed — no hosted registry left to guard)
2. ~~State store + three-strike hysteresis + geo-aware liveness policy~~ (done)
3. ~~Nightly diff summary + anomaly-only issues~~ (removed alongside Step 1)
4. ~~AI enrichment overlay + weekly delta job~~ (done)

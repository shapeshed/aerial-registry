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

## Step 2 — station state store and prune hysteresis

Add a small SQLite database, committed nowhere, persisted between nightly runs
via `actions/cache` (or S3 alongside the registry):

```sql
CREATE TABLE station_state (
  provider     TEXT NOT NULL,
  provider_id  TEXT NOT NULL,
  first_seen   TEXT NOT NULL,   -- ISO date
  last_seen    TEXT NOT NULL,
  consecutive_failures INTEGER NOT NULL DEFAULT 0,
  source_hash  TEXT,            -- hash of provider-supplied fields
  PRIMARY KEY (provider, provider_id)
);
```

Keyed on `(provider, provider_id)` — the composite identity the app already
uses. The stashed AI experiment (`stash@{1}` on the main checkout) contains a
working `cache.rs` with this shape, including `source_hash` change detection.

Rules:

- **Three strikes before prune.** An untrusted station failing liveness is
  only dropped after three consecutive nightly failures. One bad night keeps
  the station with `consecutive_failures` incremented.
- **Geo-suspect statuses never prune.** HTTP 403/451 responses from the runner
  are recorded but never remove a station; the build machine's location is not
  the listener's.
- **Trusted providers never auto-prune.** A trusted station failing repeatedly
  opens a GitHub issue instead — it signals a provider integration bug, not a
  dead station.
- **Renames are updates, not churn.** Same `(provider, provider_id)` with a
  changed name or stream URL updates the row and flags the change in the diff
  report; `first_seen` is preserved.

## Step 3 — nightly diff report and anomaly issues

After the guard runs, diff the new registry against the previous one and write
a summary to `$GITHUB_STEP_SUMMARY`: per-provider added/removed/renamed
counts, guard interventions, liveness prune list. On anomalies only — guard
triggered, trusted station failing, provider missing entirely — open (or
update) a single GitHub issue rather than emailing every run. Quiet when
healthy.

## Step 4 — AI enrichment as a committed overlay

Revive the stashed AI experiment, but decouple model inference from the
nightly build:

- Enrichment results live in a committed file (`enrichment.toml`), keyed by
  `provider:provider_id`, holding cleaned name, tags, and description.
- The nightly build applies the overlay deterministically — no model, no
  network dependency, reproducible output.
- A weekly job re-runs the model only for stations whose `source_hash`
  changed or which are new, and opens a PR with the delta. Review is a quick
  scan of a small diff, not a re-audit of the whole registry.
- The model backend is any OpenAI-compatible endpoint via `AERIAL_AI_URL` /
  `AERIAL_AI_MODEL`: a local `llama-server` (see `docs/local-ai-llamacpp.md`)
  for development, or Claude Haiku in CI using the existing
  `ANTHROPIC_API_KEY` secret from `discover.yml`.

Current model findings (from the stashed evaluation): Gemma is strongest on
tags and descriptions, Llama best preserves public station titles. The parser
already tolerates messy model output; remaining work is prompt tuning.

## Order of work

1. ~~Previous-registry guard~~ (this PR)
2. State store + three-strike hysteresis + geo-aware liveness policy
3. Nightly diff summary + anomaly-only issues
4. Un-stash the AI work onto a branch, refit as overlay + weekly delta job

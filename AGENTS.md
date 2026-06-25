# Agents

## Mission

Find and play internet radio fast.

## Background

Aerial is an internet radio player for Android. The source code is at ../aerial.
This repository builds an offline registry of radio stations that ships with
Aerial, giving users a reliable, fast-loading catalogue without depending on
network availability at app launch.

The registry is built nightly in Rust, querying multiple public radio APIs,
normalising results into a shared format, and writing a compressed JSON file
ready to bundle with the app. It must support searching by station name,
country, and tags via a SQLite FTS5 index in the Android app.

## Provider Agent Contract

A provider agent must:

- Fetch station data from one or more public APIs (no HTML scraping, no
  authenticated endpoints).
- Return a list of station records normalised to a shared internal structure.
- Handle its own errors internally, returning a partial result rather than
  propagating a failure that aborts the whole pipeline.
- Not deduplicate across providers or check stream liveness — those are pipeline
  responsibilities.

What a provider agent may do:

- Produce more than one record per source entry (e.g. multiple stream variants).
- Make secondary requests to enrich data (e.g. resolving a stream URL from a
  manifest, or fetching a logo).
- Call another provider's API as a metadata source (e.g. looking up a favicon
  from Radio Browser for a station discovered elsewhere).

## Known Providers

| Slug           | Documentation                    |
| -------------- | -------------------------------- |
| `ard`          | `docs/providers/ard.md`          |
| `bbc`          | `docs/providers/bbc.md`          |
| `bauer`        | `docs/providers/bauer.md`        |
| `curated`      | reads `stations.toml` at root    |
| `global`       | `docs/providers/global.md`       |
| `radio-france` | `docs/providers/radio-france.md` |
| `rtve`         | `docs/providers/rtve.md`         |
| `wireless`     | `docs/providers/wireless.md`     |

Adding a new broadcaster provider means: writing a `docs/providers/{slug}.md`,
implementing the provider in `src/providers/`, and registering it in the pipeline.

The `curated` provider is different — it reads `stations.toml` at the repo root.
Independent stations that are not covered by a broadcaster provider go there.

## Station Discovery Workflow

To discover new station candidates and propose additions to `stations.toml`:

```
pip install requests
python scripts/discover.py --countries GB DE FR ES --output proposed.toml
```

This queries Radio Browser, applies mechanical quality filters, and fetches
logos. It writes every candidate that passed those filters to `proposed.toml`
with a `# votes=N bitrate=Nk` comment for context.

The reviewing agent then:
1. Opens `proposed.toml`.
2. Removes entries with junk/spammy names, pure aggregator stations, or
   stations geographically misleading for the listed country code.
3. Cleans station names: proper capitalisation, remove codec suffixes like
   `[MP3]`, `(128k)`, strip leading/trailing punctuation or symbols.
4. Appends approved entries to `stations.toml`.
5. Deletes `proposed.toml`.
6. Opens a pull request titled `feat: add curated station candidates`.

## Pipeline Agents

Pipeline agents run sequentially after all provider agents have completed.

---

### Deduplicator

Merges records that refer to the same station. Two records are duplicates if
their stream URLs normalise to the same value (lowercase, strip scheme, strip
trailing slash, drop query string).

Merge strategy:

- Keep the record with the richer data (most non-empty fields wins).
- When a station appears in both a broadcaster-direct provider and an
  aggregator, prefer the broadcaster-direct name and logo.
- Union the tags arrays across all duplicate records.

---

### Liveness Checker

Issues a HEAD request (or GET with early abort) to each stream URL. Remove any
station whose URL returns a non-2xx response or times out within 10 seconds. Run
checks concurrently with a bounded concurrency limit. Log removed stations at
WARN level with URL and failure reason.

---

### Output Writer

Serialises the deduplicated, liveness-checked station list to a compressed JSON
file at the repo root. The exact output format and field names are for the
implementing agent to design based on the data available across providers and
the requirements of the Aerial Android app.

Log the total station count and output file size at INFO level.

## Execution Order

1. Run all provider agents concurrently. Collect their normalised records.
2. Run the Deduplicator across the combined record set.
3. Run the Liveness Checker concurrently across all remaining records.
4. Run the Output Writer.

A provider failure must not abort the pipeline. Log the error at ERROR level and
continue with whatever records were gathered from other providers.

## Constraints

- Do not scrape HTML.
- Use only public, unauthenticated APIs.
- Do not include images or stream URLs that require authentication.
- Log extensively with appropriate levels (DEBUG, INFO, WARN, ERROR).

## Implementation Notes

- **Language:** Rust.
- **Concurrency:** async tasks for provider agents and liveness checks; bound
  liveness concurrency with a semaphore.
- **Logging:** structured, level-appropriate — DEBUG per station, INFO per stage
  summary, WARN for removed stations, ERROR for provider failures.
- **Schedule:** nightly CI job.

## Done When

- `cargo build` succeeds with no warnings.
- Running the pipeline produces a non-empty compressed JSON output file.
- All records in the output have a name and a stream URL.
- Dead stream URLs are not present in the output.
- Each provider has documentation in `docs/providers/`.

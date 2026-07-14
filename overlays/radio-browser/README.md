# Radio Browser overlays

Radio Browser is community-submitted and untrusted (`trusted: false`) — the
bulk provider (`src/providers/radio_browser.rs`) applies only mechanical
filtering, no curation. When a specific station's data is wrong (a dead or
superseded stream URL, a broken logo, a wrong name), fix it here instead of
in the provider or the database: these files are a delta applied on top of
every fresh run, so a correction survives indefinitely rather than being
silently overwritten.

## Adding a correction

One file per ISO country code (e.g. `GB.toml`, `DE.toml`), so a PR only ever
touches a single country. Look up the station's `provider_id` (its Radio
Browser `stationuuid`) in the current registry output, then add:

```toml
[[station]]
provider = "radio-browser"
provider_id = "<stationuuid>"
source_hash = "<leave as an obviously-wrong placeholder like 'manual'>"
stream_url = "https://correct-stream-url.example.com/live"
```

Only set the fields you're correcting — `name`, `tags`, `description`,
`stream_url`, `logo_url` are all optional overrides, and `reject = true`
drops the station entirely (e.g. for something that shouldn't be in the
registry at all). `source_hash` doesn't need to match anything real; a
mismatch just means the pipeline logs it as "stale" so it's easy to notice
if the station's own upstream data has since moved on — it does not stop
the correction from applying.

Applied by `pipeline::overlay::apply()`, the same pass that applies
`enrichment.toml` — see `docs/maintenance-plan.md` for how the two relate.

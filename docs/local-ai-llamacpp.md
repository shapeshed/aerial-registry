# Running the enrichment model locally with llama.cpp

The AI enrichment client (currently in `stash@{1}`, revived as step 4 of
`docs/maintenance-plan.md`) speaks the OpenAI-compatible chat completions API:
it POSTs to `<AERIAL_AI_URL>/v1/chat/completions`. llama.cpp's `llama-server`
exposes exactly that endpoint, so no adapter is needed.

## Install

```sh
brew install llama.cpp
```

This installs `llama-server` (and `llama-cli`). Metal GPU acceleration is on
by default on Apple Silicon.

## Download and serve a model

`llama-server` can pull GGUF models straight from Hugging Face with `-hf`;
they are cached under `~/Library/Caches/llama.cpp/`. Based on the evaluation
in `docs/ai-evaluation-plan.md`, Gemma is the best starting point:

```sh
# Gemma 3 4B instruct (QAT quant, ~3 GB) — best tags/descriptions in testing
llama-server -hf ggml-org/gemma-3-4b-it-qat-GGUF --port 9000 -c 4096 --parallel 4
```

Alternatives worth comparing:

```sh
# Gemma 3 12B — same family, noticeably better quality if you have ~8 GB to spare
llama-server -hf ggml-org/gemma-3-12b-it-qat-GGUF --port 9000 -c 4096 --parallel 4

# Llama 3.1 8B instruct — best at preserving public station titles
llama-server -hf bartowski/Meta-Llama-3.1-8B-Instruct-GGUF:Q4_K_M --port 9000 -c 4096 --parallel 4

# Qwen3 4B — many tags, fluffier descriptions; needs --jinja for its chat template
llama-server -hf ggml-org/Qwen3-4B-GGUF --jinja --port 9000 -c 4096 --parallel 4
```

Flag notes:

- `--parallel N` sets concurrent request slots — match it to
  `AERIAL_AI_CONCURRENCY` or requests queue serially. The context (`-c`) is
  shared across slots, so raise it if you raise parallelism.
- `--jinja` makes the server use the model's own chat template; some models
  (Qwen, newer Mistral) need it for correct prompting.
- `-ngl 99` forces all layers onto the GPU; the brew build defaults to this
  on Apple Silicon, so you rarely need it explicitly.

## Verify the server

```sh
curl -s http://127.0.0.1:9000/v1/models | jq

curl -s http://127.0.0.1:9000/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"gemma","messages":[{"role":"user","content":"Reply with the word ok."}]}' | jq -r '.choices[0].message.content'
```

`llama-server` serves whichever model it loaded regardless of the `model`
field in the request, so `AERIAL_AI_MODEL` is effectively a label — keep it
meaningful (`gemma`, `llama`) because it names the audit output.

There is also a status page at <http://127.0.0.1:9000/> for watching slot
usage and token throughput.

## Point the pipeline at it

```sh
AERIAL_PROVIDERS=radio-browser \
RADIO_BROWSER_COUNTRIES=MX \
AERIAL_AI_URL=http://127.0.0.1:9000 \
AERIAL_AI_MODEL=gemma \
AERIAL_AI_LIMIT=100 \
AERIAL_AI_CONCURRENCY=4 \
AERIAL_AI_AUDIT=data/review/gemma.jsonl \
AERIAL_SKIP_LIVENESS=1 \
RUST_LOG=info,aerial_registry::pipeline::ai=debug \
cargo run
```

- `AERIAL_AI_URL` — server base URL; `/v1/chat/completions` is appended
  automatically (a URL already ending in `/chat/completions` is used as-is).
- `AERIAL_AI_API_KEY` — optional; unnecessary for a local server, required
  when pointing at a hosted endpoint.
- `AERIAL_AI_LIMIT` — cap the number of stations sent to the model; keep it
  around 100 for prompt-tuning iterations.
- `AERIAL_AI_AUDIT` — JSONL audit of every assessment (old vs new
  name/tags/description plus the model's reason), for scoring with
  `scripts/score_ai_audit.py` and manual review with `jq`.

The client sends `temperature: 0`, so runs over the same sample are
comparable between models.

## Sizing expectations

On an Apple Silicon Mac, Gemma 3 4B Q4 handles roughly 25–50 stations/minute
at `--parallel 4`; the 12B model is around a third of that. A weekly delta of
changed stations (the target workflow in the maintenance plan) is typically a
few dozen stations, i.e. a couple of minutes of local inference.

# Serving clef-flash (D1, #109)

**Path that works: llama.cpp `llama-server`, endpoint `POST /v1/systemone`.** Ollama and llama-swap were not tried: `/v1/systemone` is native to llama.cpp (PR #29831, merged 2026-10-03), and a server running a clef model serves only that endpoint (no text generation). Builds on big older than that PR (b8952, `cuda-ece963`, `~/.local/llama.cpp/src`) have no `systemone` string and cannot serve it.

|Item|Value|
|---|---|
|Server|llama.cpp release b11982 (`8a199ad31`), `llama-b11982-bin-ubuntu-cuda-12.8-x64` plus `cudart-llama-b11982-bin-ubuntu-cuda-12.8-x64` (without cudart on `LD_LIBRARY_PATH` the CUDA backend silently does not load and it runs on CPU: 14 s per request)|
|Model|`ggml-org/Clef-Flash-GGUF`: `Clef-Flash-Q4_K_M.gguf` (6.7 GB) and `mmproj-Clef-Flash-Q8_0.gguf` (0.6 GB; needed for images)|
|Command|`tools/decide/serve-clef-flash.sh` (`gpu-tenant run 3d … llama-server -ngl 99 -c 8192 --ubatch-size 4096 --port 8093 --alias clef-flash`)|
|Memory|about 7.4 GB VRAM (11.7 GB total on the card with HomeStill's olmOCR/bge-m3 still resident at 4.3 GB; the card was 19 GB used before the lease), so it fits beside big's other tenants only after the lease drains the 27B coder|
|Latency, fixture (image + 3 questions, 619 input tokens)|first call 0.70 s; five warm calls 0.185–0.195 s (median 0.187 s); load takes about 5 s|

Notes: image tokens are capped at 1024 by `n_ubatch` (server log warns); a clef request reads all questions jointly in one prompt. `/v1/models` reports `output_modalities` containing `decisions`.

## The request shape, measured 2026-11-02

Four 400s to get here, so it is worth writing down. `POST /v1/systemone`:

```json
{"model":"clef-flash","state":"The cat sat on the mat.",
 "questions":{"q1":{"type":"noul","instructions":"Answer whether the state is about an animal."}}}
```

answers

```json
{"model":"clef-flash","answers":{"q1":{"type":"noul":0.9703}},
 "usage":{"input_tokens":153,"output_tokens":0}}
```

- **`state` is required** (`"state" must be provided`).
- **`questions` is an object keyed by id**, not an array (`"questions" must be a non-empty object`).
- **each question takes `instructions`**, not `prompt` (`questions.q1: "instructions" must be provided`).
  `choice` and `score` add `criteria` (a map, and a list, respectively) — `crates/bevy_oracle/src/types.rs`.
- `images` is a list of data URLs, at most 4.
- `output_tokens: 0` every time: the whole point of a decision model is that there is nothing to parse.

`bevy_oracle`'s `Request` already emits exactly this, and `Config::default()` points at
`http://127.0.0.1:8093/v1/systemone`, so a pass does not build the body by hand.

## What was actually missing on 2026-11-02

Nothing, which is the finding. Issue comments on #112 and #113 said the model "cannot be served from
this clone" because `~/clef_serve` is an empty venv with no server module, no `clef-flash` on `PATH`,
and `:8093` refusing. All true, and all beside the point: `~/clef_serve` was never the mechanism. The
pieces this document names were already on the machine — `~/.local/llama.cpp/w3-b11982`
(`llama-server --version` → `0.8.0-dev (build 11982, commit 8a199ad31)`) and the GGUF pair in the
HuggingFace cache. `bash tools/decide/serve-clef-flash.sh` came up in about 45 s and answered on the
first correct request.

One trap for whoever checks this next: `strings llama-server | grep systemone` returns **0** even on
the build that has it, because the binary is a thin 17 KB ELF and the endpoint strings live in
`libllama-server.so` beside it. Verify with `--version`, not with `strings`.

# Architecture

A map of the codebase for anyone (human or model) picking this up cold.
See [`VISION.md`](VISION.md) for the why and [`BACKLOG.md`](BACKLOG.md) for
what's built vs. planned.

## Crate layout

```
crates/
  auto-quantize-core/   # pure decision engine — no HTTP, no OS calls
    src/
      hardware.rs        # HardwareProfile: vram/ram/bandwidth snapshot
      quant.rs           # QuantOption: name + size_bytes
      architecture.rs    # ModelArchitecture: layers/hidden_size -> KV bytes
      decision.rs         # recommend(hardware, options) -> Recommendation
  auto-quantize-cli/     # the `auto-quantize` binary
    src/
      main.rs             # clap CLI, subcommand dispatch, recommend flow
      errors.rs           # AppError + exit-code contract
      probe/              # hardware probing, one backend per OS
        mod.rs             # cfg-gated dispatch
        linux.rs           # real: /proc/meminfo + nvidia-smi
        fallback.rs        # macOS/Windows stub (honest "unknown", not a guess)
      catalog/            # HuggingFace GGUF catalog lookup
        mod.rs
        parse.rs           # pure: tree-JSON -> CatalogQuant (unit-tested)
        fetch.rs           # thin: live HTTP call using parse.rs
        architecture.rs    # best-effort config.json -> ModelArchitecture,
                           #   with base_model-tag fallback (--context)
      download.rs          # streams recommended file(s) to disk, with resume
```

`auto-quantize-core` is deliberately network- and OS-free so the fit-scoring
logic (the part worth testing thoroughly) is trivial to unit-test and could
be reused by a future GUI or library consumer without dragging in HTTP or
platform probing.

## Data flow: `auto-quantize recommend <repo>`

1. `probe::probe()` — reads `/proc/meminfo` and shells out to `nvidia-smi`
   (Linux only today; other platforms get an honest all-unknown profile).
   Runs in well under a second; see the `probe_completes_in_under_one_second`
   test.
2. `catalog::fetch_quants(repo)` — calls
   `GET https://huggingface.co/api/models/{repo}/tree/main`, parses the file
   tree, filters to `.gguf` entries, and sums multi-part splits
   (`*-00001-of-00003.gguf`) into one logical `CatalogQuant` (a
   `QuantOption` for the decision engine plus the underlying file list for
   downloading). Distinguishes repo-not-found / no-gguf-files / network
   errors as distinct `CatalogError` variants.
3. If `--context <n>` was given, `catalog::fetch_architecture(repo)` tries
   the repo's own `config.json`, then falls back to the `config.json` of the
   repo named in its `base_model:<org>/<name>` tag (GGUF quant repos rarely
   publish their own full config). Failure of any kind is not fatal — it
   yields `None` and a one-line stderr note, not an error.
4. `auto_quantize_core::recommend_with_context(&hardware, &options, ..., context)`
   picks the largest quant that fits the accelerator budget (VRAM, or free
   RAM if no GPU) with headroom reserved for context/KV cache — an *exact*
   KV-cache byte count when `context` resolved, otherwise the flat 15%
   fallback — or the smallest available quant with an "expect swapping"
   reason if nothing fits.
5. Prints the quant + one-line reason (`--json` for machine-readable output).
6. On confirmation (`--yes`, or an interactive `Y`), `download::download_files`
   streams each backing file from
   `https://huggingface.co/{repo}/resolve/main/{path}` to `--output`,
   resuming from any existing partial file via an HTTP `Range` request, and
   verifying the final byte count against the size HuggingFace reported.

Errors surface through `errors::AppError`, which gives each failure class
(`Network`, `RepoNotFound`, `NoGgufFiles`, `Download`) its own stable exit
code (2-5) so scripts can branch on `$?` — see `errors.rs` tests for the
current mapping.

## Running it

```
just check     # fmt --check, clippy -D warnings, test --workspace
just build     # cargo build --workspace
just test      # cargo test --workspace
```

`cargo test --workspace` includes a couple of tests that hit the live
HuggingFace API (a known-empty and a known-populated repo) rather than
mocking it — see `docs/VISION.md`'s "live API call, not hardcoded" v1
requirement. They need network access; there's no offline test profile yet.

## Known gaps (tracked in `docs/BACKLOG.md`)

- macOS/Windows hardware probing are stubs (`probe::fallback`), not real
  backends — stories 1.3/1.4.
- `fetch_architecture`'s base-model fallback only follows one hop and only
  recognizes `transformers`-style / GPT-2-style config field names; a repo
  whose base model is itself gated, private, or unusually shaped falls back
  to the flat headroom fraction rather than erroring — this is intentional
  (docs/VISION.md's "honest about uncertainty"), not a bug to fix later.

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
      download.rs          # streams recommended file(s) to disk
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
3. `auto_quantize_core::recommend(&hardware, &options)` — picks the largest
   quant that fits the accelerator budget (VRAM, or free RAM if no GPU) with
   headroom reserved for context/KV cache, or the smallest available quant
   with an "expect swapping" reason if nothing fits.
4. Prints the quant + one-line reason (`--json` for machine-readable output).
5. On confirmation (`--yes`, or an interactive `Y`), `download::download_files`
   streams each backing file from
   `https://huggingface.co/{repo}/resolve/main/{path}` to `--output`,
   verifying the downloaded byte count against the size HuggingFace reported.

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
- No context-length-aware KV-cache headroom yet; `decision.rs` reserves a
  flat 15% of budget — story 1.6.
- No download resume support — story 2.2.
- No `--reserve-vram` / `--context` / `--prefer` override flags yet —
  story 3.3.

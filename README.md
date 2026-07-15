# auto-quantize

[![CI](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml/badge.svg)](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Stop guessing which quant fits your machine.** One command benchmarks your
hardware and tells you — then downloads — the best-fitting quantized build of
any open model on HuggingFace, instead of you eyeballing a GGUF repo and
re-downloading three times until something finally loads without swapping to
death.

```
$ auto-quantize recommend TheBloke/Llama-3-8B-GGUF
Probing hardware... done (0.31s)
  GPU:  Apple M2 Pro, 16 GB unified memory (≈11.5 GB usable)
  RAM:  32 GB system, 18 GB free
  Bus:  unified memory, ~200 GB/s

Recommendation: Q5_K_M (5.7 GB)
  Fits entirely in VRAM with 5.8 GB headroom for context + KV cache.
  Q6_K would spill 1.2 GB to system RAM — noticeably slower on this bus.

Download this build? [Y/n]
```

No menus, no spreadsheet math, no "just try Q4 and see." One process reads
your machine, one model reads the tradeoffs, one line explains the answer.

## Why

Anyone running local LLMs hits the same wall: model cards list a dozen GGUF
quantizations and offer no guidance on which one your specific machine can
actually run well. The usual workflow is download-run-OOM-repeat, burning
bandwidth and time on trial and error. The math to avoid this — VRAM budget,
KV cache overhead, quality-per-bit tradeoffs — is well understood, but nobody
has packaged it as a five-second CLI check.

`auto-quantize` reuses the sizing math from
[Fit Check](https://github.com/ctkrug/fit-check) and pairs it with real,
cross-platform hardware probing so the decision is made *for* you, with a
reason you can sanity-check in one line.

## Planned features

- **Hardware probe** — detect GPU/VRAM (NVIDIA, Apple Silicon, AMD where
  available), system RAM, and effective memory bandwidth in well under a
  second, on Linux, macOS, and Windows.
- **Quant catalog lookup** — pull the list of available GGUF quantizations
  for any HuggingFace model repo without downloading the weights themselves.
- **Decision engine** — score each available quant against the probed
  hardware (fits fully in VRAM > fits with partial offload > swaps), reusing
  Fit Check's sizing formulas, and pick a winner with a one-line rationale.
- **Download** — fetch the recommended file directly, with resume support
  and a progress bar, no browser or account required.
- **Scriptable output** — `--json` for piping into other tooling, a
  non-interactive `--yes` flag, and a proper exit code contract for CI use.
- **Override knobs** — `--reserve-vram`, `--context <n>`, and `--prefer
  quality|speed` for users who want to nudge the recommendation instead of
  fighting it.

## Stack

Rust, dependency-light by design:
- `clap` for the CLI surface
- `reqwest` (blocking, rustls) for HuggingFace API calls and downloads
- `serde` / `serde_json` for the quant catalog and `--json` output
- Platform-native hardware probing (no bundled GPU vendor SDKs) with thin,
  swappable OS backends for Linux/macOS/Windows

See [`docs/VISION.md`](docs/VISION.md) for the full design rationale and
[`docs/BACKLOG.md`](docs/BACKLOG.md) for the build plan.

## Status

Early scaffold — not yet functional. See the backlog for what's next.

## License

MIT — see [LICENSE](LICENSE).

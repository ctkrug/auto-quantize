# auto-quantize

[![CI](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml/badge.svg)](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Stop guessing which quant fits your machine.** One command benchmarks your
hardware and tells you — then downloads — the best-fitting quantized build of
any open model on HuggingFace, instead of you eyeballing a GGUF repo and
re-downloading three times until something finally loads without swapping to
death.

```
$ auto-quantize recommend TheBloke/Llama-2-7B-Chat-GGUF
Probing hardware...
Recommendation: Q4_K_M (4.1 GB)
  fits entirely within budget with 2.3 GB headroom for context + KV cache
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

## Usage

```
auto-quantize recommend <hf-repo>            # probe, fetch, recommend, prompt to download
auto-quantize recommend <hf-repo> --yes      # skip the confirmation and download immediately
auto-quantize recommend <hf-repo> --json     # single JSON object on stdout, no prompt
auto-quantize recommend <hf-repo> --timing   # print hardware-probe latency to stderr
auto-quantize recommend <hf-repo> -o <dir>   # download into <dir> instead of the cwd
auto-quantize recommend <hf-repo> --reserve-vram 2   # reserve 2 extra GB of headroom
auto-quantize recommend <hf-repo> --prefer speed     # step down one size for extra margin
auto-quantize recommend <hf-repo> --context 8192     # size headroom for an 8192-token context
auto-quantize probe                          # print the detected hardware profile and exit
```

Exit codes: `0` success, `2` network error, `3` repo not found, `4` no GGUF
quantizations in the repo, `5` download failed (e.g. size mismatch).

## Features

- **Hardware probe** — real on Linux (`/proc/meminfo` for RAM, `nvidia-smi`
  for VRAM), macOS (`sysctl`/`vm_stat` for RAM; unified memory or
  `system_profiler` for VRAM), and Windows (`GlobalMemoryStatusEx` for RAM,
  DXGI adapter enumeration for VRAM) — in well under a second on every
  platform.
- **Quant catalog lookup** — pulls the live list of `.gguf` files for any
  HuggingFace model repo (name + size, no weight download), grouping
  multi-part splits into one logical quant option.
- **Decision engine** — scores each available quant against the probed
  hardware (fits fully > swaps), reserving headroom for context/KV cache,
  and picks a winner with a one-line rationale.
- **Download** — fetches the recommended file(s) with a progress indicator,
  resumes an interrupted download from where it left off (HTTP `Range`
  request) instead of restarting from zero, and verifies the final size
  against HuggingFace's reported size.
- **Scriptable output** — `--json` for piping into other tooling, a
  non-interactive `--yes` flag, and a distinct exit code per failure class.
- **Override flags** — `--reserve-vram <GB>` to pad the headroom beyond the
  default, `--prefer quality|speed` to break ties toward the largest fitting
  quant or one size down for extra margin, `--context <n>` to size headroom
  from an exact KV-cache calculation instead of the flat 15% fallback.
- **Context-aware headroom** — `--context <n>` resolves the repo's model
  architecture (its own `config.json`, or its tagged base model's) and
  reserves exactly the KV-cache bytes that context length needs, rather
  than a flat fraction of the budget. Falls back to the flat reservation,
  with a one-line note, when no architecture can be resolved.

### Planned

- Effective memory-bandwidth probing (today: VRAM/RAM budget only, no
  throughput estimate).

## Stack

Rust, dependency-light by design:
- `clap` for the CLI surface
- `reqwest` (blocking, rustls) for HuggingFace API calls and downloads
- `serde` / `serde_json` for the quant catalog and `--json` output
- Platform-native hardware probing (no bundled GPU vendor SDKs) with thin,
  swappable OS backends for Linux/macOS/Windows

See [`docs/VISION.md`](docs/VISION.md) for the full design rationale,
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how the code fits
together, and [`docs/BACKLOG.md`](docs/BACKLOG.md) for the build plan.

## Status

The core loop works end to end on Linux, macOS, and Windows: real hardware
probing, a live HuggingFace catalog fetch, the fit-scoring decision engine,
and a resumable download of the recommended file. See the backlog for the
remaining polish (effective memory-bandwidth probing).

## License

MIT — see [LICENSE](LICENSE).

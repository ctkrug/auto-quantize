# Snug

**▶ Live: [apps.charliekrug.com/snug/](apps.charliekrug.com/snug/)**

[![CI](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml/badge.svg)](https://github.com/ctkrug/auto-quantize/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**The quant that fits your machine, first try.** One command probes your
hardware, reads the GGUF file list for any HuggingFace model, and downloads the
largest quant that actually fits, so you stop eyeballing a repo and
re-downloading three times until something loads without swapping to death.

```
$ snug recommend TheBloke/Llama-2-7B-Chat-GGUF
Probing hardware...
Recommendation: Q5_K_M (5.1 GB)
  fits entirely within budget with 3.4 GB headroom for context + KV cache
Download this build? [Y/n]
```

No menus, no spreadsheet math, no "just try Q4 and see." One process reads your
machine, one line explains the answer, one keypress downloads it.

## Who it's for

Anyone running local LLMs through the llama.cpp family (Ollama, LM Studio,
koboldcpp, raw llama.cpp) who wants a straight answer instead of a guess. Every
GGUF repo lists a dozen quantizations and offers no help picking the one your
specific machine can run well, so the usual workflow is download, run, OOM,
repeat, burning bandwidth on trial and error. The math to skip that (VRAM
budget, KV-cache overhead, quality-per-bit tradeoffs) is well understood, but
nobody had packaged it as a five-second CLI check. Snug is that check.

## Install

```
cargo install --git https://github.com/ctkrug/auto-quantize
```

One static binary, no runtime. Builds on Linux, macOS, and Windows.

## Usage

```
snug recommend <hf-repo>            # probe, fetch, recommend, prompt to download
snug recommend <hf-repo> --yes      # skip the confirmation and download immediately
snug recommend <hf-repo> --json     # single JSON object on stdout, no prompt
snug recommend <hf-repo> --timing   # print hardware-probe latency to stderr
snug recommend <hf-repo> -o <dir>   # download into <dir> instead of the cwd
snug recommend <hf-repo> --reserve-vram 2   # reserve 2 extra GB of headroom
snug recommend <hf-repo> --prefer speed     # step down one size for extra margin
snug recommend <hf-repo> --context 8192     # size headroom for an 8192-token context
snug probe                          # print the detected hardware profile and exit
```

`snug probe` prints what Snug sees on this machine:

```
$ snug probe
Hardware profile:
  VRAM:       12.0 GB
  RAM total:  32.0 GB
  RAM free:   18.4 GB
  Bandwidth:  unknown
```

Exit codes: `0` success, `2` network error, `3` repo not found, `4` no GGUF
quantizations in the repo, `5` download failed (e.g. size mismatch). Each
failure class has its own code so a script can branch on `$?` without parsing
text.

## Features

- **Hardware probe**: real on Linux (`/proc/meminfo` for RAM, `nvidia-smi`
  for VRAM), macOS (`sysctl`/`vm_stat` for RAM; unified memory or
  `system_profiler` for VRAM), and Windows (`GlobalMemoryStatusEx` for RAM,
  DXGI adapter enumeration for VRAM). Under a second on every platform, no
  vendor SDK required.
- **Quant catalog lookup**: pulls the live list of `.gguf` files for any
  HuggingFace model repo (name and size, no weight download), grouping
  multi-part splits into one logical quant option.
- **Decision engine**: scores each available quant against the probed
  hardware (fits fully beats swaps), reserves headroom for context and KV
  cache, and picks a winner with a one-line rationale.
- **Download**: fetches the recommended file(s) with a progress indicator,
  resumes an interrupted download from where it left off (HTTP `Range`
  request) instead of restarting from zero, and verifies the final size
  against HuggingFace's reported size.
- **Scriptable output**: `--json` for piping into other tooling, a
  non-interactive `--yes` flag, and a distinct exit code per failure class.
- **Override flags**: `--reserve-vram <GB>` to pad the headroom, `--prefer
  quality|speed` to break ties toward the largest fitting quant or one size
  down for extra margin, `--context <n>` to size headroom from an exact
  KV-cache calculation instead of the flat 15% fallback.
- **Context-aware headroom**: `--context <n>` resolves the repo's model
  architecture (its own `config.json`, or its tagged base model's) and
  reserves exactly the KV-cache bytes that context length needs. Falls back to
  the flat reservation, with a one-line note, when no architecture resolves.

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

The fit-scoring logic lives in a separate, network-free `auto-quantize-core`
crate so the part worth testing thoroughly is trivial to unit-test. See
[`docs/VISION.md`](docs/VISION.md) for the design rationale,
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how the code fits together,
and [`docs/BACKLOG.md`](docs/BACKLOG.md) for the build plan.

## Status

The core loop works end to end on Linux, macOS, and Windows: real hardware
probing, a live HuggingFace catalog fetch, the fit-scoring decision engine, and
a resumable download of the recommended file. See the backlog for the remaining
polish (effective memory-bandwidth probing).

## License

MIT licensed. See [LICENSE](LICENSE).

---

More of Charlie's projects → [apps.charliekrug.com](https://apps.charliekrug.com)

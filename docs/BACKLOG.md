# Backlog

Epics and stories for the v1 build. All start unchecked. Every story lists
concrete, verifiable acceptance criteria — no "works well" vibes.

See [`docs/VISION.md`](VISION.md) for the why behind these choices.

## Epic 1 — Core recommendation flow

The wow moment and the real hardware/catalog plumbing behind it.

- [x] **1.1 [WOW] `recommend` prints one quant + one reason, end to end**
  - Running `auto-quantize recommend <hf-repo>` against a real public GGUF
    repo (e.g. a small TheBloke/-style repo) prints exactly one recommended
    quant name and exactly one reason line to stdout, with no interactive
    menu or prompt shown before the recommendation appears.
  - The hardware-probing portion of the run (excluding HTTP calls) completes
    in under 1 second, verified by an internal timing check surfaced with
    `--timing` or logged in a benchmark test.
  - Pointing the command at a repo with zero GGUF files exits non-zero with
    a one-line, non-panicking error message.

- [x] **1.2 Real Linux hardware probing**
  - On a Linux host with an NVIDIA GPU, `auto-quantize probe` reports a
    non-zero `vram_bytes` sourced from `nvidia-smi` (or equivalent driver
    query), not a hardcoded value.
  - On a Linux host with no discrete GPU, `probe` reports `vram_bytes: null`
    and falls back to system RAM, without erroring.
  - `ram_bytes` and `ram_free_bytes` match `/proc/meminfo` (`MemTotal` /
    `MemAvailable`) within rounding error, verified against a fixture file
    in a unit test.

- [ ] **1.3 Real macOS hardware probing**
  - On Apple Silicon, `probe` reports unified memory size via `sysctl`
    (`hw.memsize`) as both `ram_bytes` and an inferred `vram_bytes` budget,
    with a comment/doc note explaining the unified-memory assumption.
  - On Intel Macs with a discrete GPU, `vram_bytes` is sourced from
    `system_profiler SPDisplaysDataType` (or documented as unsupported with
    a graceful `None` fallback — not a crash).

- [ ] **1.4 Real Windows hardware probing**
  - `probe` reports `ram_bytes`/`ram_free_bytes` via `GlobalMemoryStatusEx`
    and `vram_bytes` via DXGI adapter enumeration.
  - Running on a Windows CI runner (per the CI matrix) exercises this path
    and the command exits 0.

- [x] **1.5 Fetch real GGUF quant catalog from HuggingFace**
  - Given a HuggingFace repo id, the tool lists every `.gguf` file in the
    repo (name + byte size) via the HuggingFace API, without downloading
    any file content.
  - A repo with multiple quant "buckets" packed as multi-part files (e.g.
    `-00001-of-00002.gguf`) is either summed into one logical quant option
    or clearly listed as multi-part — not silently mis-sized.
  - Network failure (timeout, 404, rate limit) produces a specific,
    distinguishable error message per case, not one generic "failed".

- [x] **1.6 Context-aware headroom in the decision engine**
  - `recommend` accepts an implied or default context length and computes
    KV-cache headroom from it (model layer count / hidden size where
    available) instead of today's flat 15%-of-budget placeholder.
  - Unit tests cover: small context fits with room to spare, large context
    pushes a previously-fitting quant into the next size down, and the
    reason string names the context length when it's the limiting factor.

## Epic 2 — Download experience

- [x] **2.1 Download the recommended file**
  - Accepting the recommendation (`Y` at the prompt, or `--yes`) downloads
    the exact recommended `.gguf` file to the current directory (or
    `--output <dir>`), with a visible progress indicator.
  - The downloaded file's byte size matches the size reported by the
    HuggingFace API for that file.

- [ ] **2.2 Resume interrupted downloads**
  - Killing the process mid-download and re-running the same command
    resumes from the existing partial file (HTTP range request) instead of
    restarting from zero, verified by asserting the transferred-byte count
    on resume is less than the full file size.

- [x] **2.3 Graceful handling of missing/incompatible models**
  - A repo id that doesn't exist on HuggingFace produces a clear "repo not
    found" error and a non-zero exit code, not a stack trace.
  - A repo that exists but has no `.gguf` files at all produces a clear
    "no GGUF quantizations found" error distinct from the not-found case.

## Epic 3 — Scriptability and polish

- [x] **3.1 `--json` machine-readable output**
  - `auto-quantize recommend <repo> --json` emits a single JSON object to
    stdout with `hardware`, `recommendation`, and `reason` fields and no
    other stdout output mixed in (human-readable text goes to stderr, if
    printed at all).
  - The JSON output round-trips through `serde_json` in a test (parse it
    back into a typed struct without error).

- [x] **3.2 Non-interactive flag and exit code contract**
  - `--yes` skips the download confirmation prompt entirely.
  - Exit code 0 on a successful recommend-and-download; a documented
    non-zero code on each distinct failure class (network error, repo not
    found, no compatible quants) — verified by an integration test
    asserting specific codes per case.

- [ ] **3.3 Override flags for power users**
  - `--reserve-vram <GB>` increases the reserved headroom beyond the
    default and visibly changes which quant is recommended in a
    before/after test.
  - `--context <n>` overrides the assumed context length used by the
    headroom calculation from story 1.6.
  - `--prefer quality|speed` breaks ties between two similarly-fitting
    quants in the documented direction (quality picks the larger fitting
    option; speed picks the smaller one for extra headroom), covered by a
    unit test with two quants that both fit.

- [x] **3.4 Polished `--help` and error output**
  - `auto-quantize --help` and `auto-quantize recommend --help` document
    every flag above with a one-line description, generated by `clap`
    (no hand-maintained help text to drift out of sync).
  - Every user-facing error message is a complete sentence with no raw
    Rust `Debug` output (e.g. no leaked `Err(...)` or backtrace) unless
    `--verbose` is passed.

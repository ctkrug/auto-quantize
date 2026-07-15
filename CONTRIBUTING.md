# Contributing

## Build and test

```sh
cargo build --workspace
cargo test --workspace
```

## Before opening a PR

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Both run in CI (`.github/workflows/ci.yml`) across Linux, macOS, and Windows,
so a clean run locally on one platform doesn't guarantee a clean run in CI —
check the Actions tab if you don't have access to all three.

## Project layout

- `crates/auto-quantize-core` — hardware/quant types and the fit-scoring
  decision engine. No network or OS calls; keep it that way so it stays
  fast to unit-test.
- `crates/auto-quantize-cli` — the `auto-quantize` binary: argument parsing,
  platform hardware probing, HuggingFace API calls, and downloads.

See [`docs/VISION.md`](docs/VISION.md) for the design rationale and
[`docs/BACKLOG.md`](docs/BACKLOG.md) for the current story queue.

# Vision

## The problem

Every GGUF repo on HuggingFace lists a dozen quantizations —
`Q2_K`, `Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, and so on — with no guidance on
which one a given machine can actually run well. The model card explains
what each quant *is* (a bits-per-weight and quality tradeoff) but never what
it means *for you*. The result is a familiar, wasteful loop:

1. Guess a quant based on vague vibes ("Q4 is probably safe").
2. Download several GB.
3. Load it. It OOMs, or it loads but swaps to disk and crawls.
4. Delete it, guess smaller, repeat.

The information needed to skip straight to the right answer already exists —
available VRAM/RAM, memory bandwidth, the quant's file size, and rough KV
cache overhead are all either measurable or published. Nobody has wired them
together into a five-second check.

## Who it's for

Anyone running local LLMs via `llama.cpp`-family tooling (Ollama, LM Studio,
koboldcpp, raw llama.cpp) who wants a straight answer instead of a spreadsheet:
hobbyists picking their first local model, developers scripting model
selection into a setup process, and power users juggling several machines
(a beefy desktop, a laptop, a home server) who don't want to re-derive the
right quant for each one by hand.

## The core idea

One CLI command:

```
snug recommend <hf-repo>
```

1. **Probe** the local machine's hardware in well under a second: VRAM (or
   unified memory), system RAM, and an estimate of effective memory
   bandwidth.
2. **Fetch** the list of available GGUF quantizations for `<hf-repo>` from
   the HuggingFace API — file names and sizes only, no weight download.
3. **Score** each quant against the hardware profile using sizing math
   ported from [Fit Check](https://github.com/ctkrug/fit-check): does it fit
   entirely in the accelerator budget (with headroom for context + KV
   cache), does it fit with partial CPU offload, or does it require swapping?
4. **Recommend** the best-fitting option with a one-line reason, then offer
   to download it.

No interactive menus, no configuration file to fill out first. The default
path is: run the command, read one line, say yes.

## Key design decisions

- **Rust, not Python.** The tool needs to run in well under a second with no
  runtime installed and no dependency hell — a single static-ish binary you
  can drop anywhere. Rust also gives direct access to platform APIs for
  hardware probing without shelling out to vendor tools where avoidable.
- **Reuse Fit Check's math, don't re-derive it.** The VRAM-budget and
  headroom-for-context reasoning already exists and is proven; the new work
  here is the hardware probing and the CLI/download experience around it,
  not re-inventing the sizing formulas.
- **Vendor-agnostic hardware probing.** No bundled CUDA/ROCm/Metal SDKs.
  Prefer OS-level and driver-adjacent signals (e.g. `nvidia-smi` output
  parsing, `sysctl`/Metal on macOS, `/proc` + vendor sysfs on Linux) so the
  binary stays dependency-light and the probe stays fast.
- **Core logic is a separate, network-free library crate.** `auto-quantize-core`
  takes a `HardwareProfile` and a `Vec<QuantOption>` and returns a
  `Recommendation` — no HTTP, no OS calls. This keeps the decision engine
  (the part with real logic worth testing thoroughly) trivial to unit-test
  and reusable if a GUI or library-only use case ever wants it directly.
- **Scriptable by default.** `--json` output and a meaningful exit code
  contract from day one, because "vendor-agnostic CLI for people who don't
  want a GUI" is the differentiator — it has to compose into other tooling,
  not just look nice interactively.
- **Honest about uncertainty.** When memory bandwidth or VRAM can't be
  determined on a given platform, the tool says so and falls back to a more
  conservative recommendation rather than pretending to know.

## What "v1 done" looks like

- Running `snug recommend <hf-repo>` on Linux, macOS, or Windows
  probes real hardware (not a stub) in well under a second and prints a
  single recommended quant with a one-line reason — the wow moment, working
  end to end, no menus.
- The recommendation is backed by the real GGUF file list for the given
  HuggingFace repo (live API call, not a hardcoded catalog).
- Accepting the recommendation downloads the file, with resume support and
  a progress indicator.
- `--json` produces machine-readable output; `--yes` skips the download
  confirmation; the process exits non-zero on any failure a script would
  need to detect (bad repo, no network, no quants found).
- The decision engine (`auto-quantize-core`) has thorough unit test coverage
  of its fit-scoring logic across the fits-fully / partial-offload /
  swap-fallback cases.
- CI is green on Linux, macOS, and Windows.

Everything past that — override flags, quality-vs-speed preference,
multi-GPU awareness — is v2 territory tracked in the backlog, not v1 scope.

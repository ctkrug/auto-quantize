---
title: "I built a CLI that picks the right GGUF quant for your machine"
published: false
tags: rust, llm, cli, localllama
---

Every GGUF repo on HuggingFace lists a dozen quantizations. `Q2_K`, `Q4_K_M`,
`Q5_K_M`, `Q6_K`, `Q8_0`, and so on. The model card tells you what each one is,
a bits-per-weight and quality tradeoff, but never what it means for the machine
in front of you. So the workflow ends up being: guess a quant, download several
gigabytes, load it, watch it either OOM or swap to disk and crawl, delete it,
guess smaller, repeat.

The information needed to skip that loop already exists. Your VRAM or free RAM
is measurable, the quant file sizes are published, and the KV-cache overhead is
a short formula. Nobody had wired them together into a five-second check, so I
did. It is called Snug, it is written in Rust, and the whole thing is one
command:

```
$ snug recommend TheBloke/Llama-2-7B-Chat-GGUF
Probing hardware...
Recommendation: Q5_K_M (5.1 GB)
  fits entirely within budget with 3.4 GB headroom for context + KV cache
Download this build? [Y/n]
```

Two build decisions turned out to matter more than I expected.

## The decision engine has no idea the network exists

The part with real logic worth testing is the scoring: given a hardware budget
and a list of quants with sizes, which one is the largest that fits after
reserving headroom? I put that in its own crate, `auto-quantize-core`, that
takes a `HardwareProfile` and a `Vec<QuantOption>` and returns a
`Recommendation`. No HTTP client, no OS calls, no `nvidia-smi`.

That separation paid off immediately. The core crate has 100% line coverage
because every case (fits fully, one size down for a speed preference, nothing
fits so fall back to the smallest, an exact KV-cache reservation for a given
context length) is a pure function call with fixed inputs. No mocking an HTTP
server to test a comparison. The CLI crate stays thin: probe the machine, fetch
the file list, hand both to core, print the answer.

## Untrusted numbers want saturating arithmetic

The KV-cache size for a context length is a product:
`2 * num_layers * hidden_size * 2 bytes * context_length`. Every one of those
inputs comes from somewhere I do not control. `num_layers` and `hidden_size`
come from a repo's `config.json`. `context_length` comes from a `--context`
flag a user types. The file sizes come from the HuggingFace API.

The first version multiplied them with plain arithmetic. Then I fed it a
`--context` of a few hundred thousand against a large model and it panicked in a
debug build on integer overflow, which in a release build would have silently
wrapped to a tiny number and recommended a quant that does not actually fit.
That is the exact failure the tool exists to prevent, caused by the tool.

The fix is to saturate everywhere an attacker-or-typo-controlled number gets
multiplied or summed:

```rust
KV_TENSORS
    .saturating_mul(self.num_layers as u64)
    .saturating_mul(self.hidden_size as u64)
    .saturating_mul(BYTES_PER_ELEMENT)
    .saturating_mul(context_length as u64)
```

Saturating to `u64::MAX` still gives the honest answer here: an absurd context
reserves all the budget, so nothing fits, so the tool says so. I found four bugs
in this family during QA (KV-cache overflow, multi-part size-sum overflow, a
stale oversized download file getting silently accepted, and a stdin EOF that
defaulted a download prompt to yes), and each one got a failing test first, then
the fix.

That last one is worth a sentence. A bare Enter keypress at the `[Y/n]` prompt
sends a newline and should mean yes. But EOF with zero bytes read, which is what
you get from a closed or piped stdin in a script that forgot `--yes`, is not the
same thing, and it must default to no. Otherwise a cron job could quietly pull a
multi-gigabyte file nobody asked for.

## What I would do differently

The one thing on the list is effective memory-bandwidth probing. Right now Snug
reasons about capacity (does it fit) but not throughput (how fast will it run).
Bandwidth is the better predictor of tokens per second once a model fits, and I
left a clean seam in the `HardwareProfile` for it, but measuring it portably
without a benchmark that takes longer than the whole rest of the command is the
hard part.

Snug is MIT licensed and runs on Linux, macOS, and Windows. Code and install
instructions are on GitHub: https://github.com/ctkrug/auto-quantize

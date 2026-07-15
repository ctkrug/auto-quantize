# Design — Snug landing page

Snug is a CLI, so this governs the **marketing landing page** (`site/index.html`),
not an in-app UI. The page markets the tool to the person who runs it. Product and
page share one brand.

## Product

- **Name:** Snug
- **Tagline:** The quant that fits your machine, first try.
- **Audience:** People running local LLMs (Ollama, LM Studio, llama.cpp, koboldcpp)
  who are tired of guessing which GGUF quant fits their box and re-downloading two
  or three times before one loads without swapping.
- **One benefit:** Run one command, download the right quant on the first try.

## 1. Aesthetic direction

Snug is a **measured blueprint notebook**: a warm graph-paper ground, ink-navy
structure lines, and one confident amber accent — the page reads like an
engineer's fit-check sketch, which is exactly what the tool does (measure the
machine, size the build to it). Deliberately *not* the terminal-green CLI cliché
and *not* the dark-gray-cards default; the warmth plus the graph grid is the
personality.

## 2. Tokens (actual values)

Color:
- `--paper` background `#f4f1e9` (warm off-white)
- `--paper-2` surface `#ece6d6`
- `--paper-3` raised surface `#e4dcc7`
- `--ink` text `#1e2740` (ink navy)
- `--ink-muted` muted text `#5b6580`
- `--accent` amber `#d97706` (mark, links, CTA)
- `--accent-soft` `#f0a94a` (hover/glow)
- `--line` blueprint line `#c9c0aa`
- `--support` ink-blue `#2f4a7a` (secondary structure)
- `--ok` `#2f7d4f`  ·  `--warn` `#b3402e`
- Dark mode: ink ground `#131a2b`, warm paper text `#efe9db`, same amber accent.

Type:
- Display: **Space Grotesk** (geometric, engineered) — wordmark + headings.
- Body: **Inter** — paragraphs, features.
- Mono: **JetBrains Mono** — the terminal sample, measured callouts, code.
- Scale ~1.25; body 16–18px; measures ≤ 68ch.

Space/shape/motion:
- 8px spacing scale.
- Radius 6px (technical, not pillowy). 1px `--line` borders carry the blueprint feel.
- Depth = a soft layered shadow + a hairline ink border, never a flat panel.
- Motion 160ms ease-out on hover/reveal; a subtle draw-in on the terminal sample.

## 3. Layout intent

- **Hero (the star):** left column = wordmark, headline, subhead, install + CTA;
  right column = a faux-terminal card showing the real `snug recommend` output.
  On desktop the hero fills the first viewport; the terminal card is the visual
  anchor (~50% width). At 390px it stacks: headline, terminal card, CTA — no dead
  space, terminal card goes full width.
- Below: a 3-up "how it works" (probe → fetch → fit), a benefits grid, a real
  sample block, install/usage, and an FAQ (the useful search-intent copy).
- Graph-paper grid is a fixed background treatment behind everything (very low
  contrast), so no surface is a flat solid to the edges.

## 4. Signature detail

The **caliper wordmark**: "Snug" set in Space Grotesk with a small amber
bracket/caliper glyph `[ ]` hugging it — the caliper is the fit-measuring
instrument, tying the name to the function. Reused as the favicon (amber caliper
+ "S" on ink).

## 5. Brand assets

- Favicon: inline SVG data-URI — amber caliper brackets around an ink "S" on paper.
- Wordmark: designed (letter-spacing, the caliper glyph in accent), not just the
  heading font.

The page passes the D4 ship gate: hero fills the viewport, themed controls/links
with real hover+focus states, composed at 390/768/1440, depth on every surface, a
real favicon, and copy that clears the anti-slop gate.

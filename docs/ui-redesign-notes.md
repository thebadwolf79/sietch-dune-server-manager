# UI redesign — parking lot (deferred until functionality is complete)

**Status:** NOT started. This is a *near-the-end* task by operator decision —
we finish the functional work (currency grants #29, DR backup #30, the live-server
investigations, etc.) first, then revisit the look-and-feel.

**Planned process (when we get here):** a **three-round discussion across all five
agent roles** on the best UI decisions, in the spirit of the Final Review Stage:
- Round 1 — each role proposes / critiques independently (Architect=Gemini deciding
  vote, Lead=Claude, QC=ChatGPT, Researcher=Perplexity, Stress-Tester=Grok).
- Round 2 — react to each other's notes; converge on a concrete theme spec.
- Round 3 — refine + sign-off (Architect), producing an actionable token set + plan.

This file is **just documentation to weigh against the full group later** — it is one
role's (Researcher / Perplexity) early input, gathered with very little context, not a
decision.

---

## Constraints the group must weigh (added by Lead, so we don't lose them)

- **Current stack is Radix Themes** (`@radix-ui/themes`) + React. Any direction is
  either a *re-theme within Radix* (tokens/accent/appearance) or a deliberate, scoped
  migration — not a from-scratch rewrite. "Wrap, don't replace" applies to our own UI too.
- **It's an operations tool first.** Readability of status, counts, IPs/ports, timestamps,
  and clear destructive-action separation outrank theming. Don't trade clarity for mood.
- **Two audiences** (recurring project theme): solo/self operator vs. a public-server
  admin. The theme shouldn't assume either; it should stay legible for long sessions.
- **Accessibility:** never rely on color alone for state; check contrast ratios on the
  proposed palette (some sand-on-charcoal pairs below may be borderline for small text).
- **No verified official font pack** was found in the Researcher's sources, so any
  "Dune font" is an approximation — pick for readability, not mimicry.
- **Don't regress current behavior** (the tunnel-backed tabs, admin command forms,
  VM power controls, etc.) when restyling.

---

## Researcher (Perplexity) input — 2026-06-10, low context

> Captured verbatim-faithful for later review. Treat as a mood/direction starting point.

**Direction:** "minimal sci-fi utility," not ornate desert fantasy. Sparse, functional
layout; strong information grid; dark charcoal backgrounds, sand text, one warm accent for
key actions/alerts. Imagery (subtle dunes, heat haze, harsh light) informs backgrounds /
dividers without clutter. Squared edges, thin borders, small radii — "engineered, not
decorative."

**Proposed palette:**

| Role | Hex |
|------|-----|
| Background | `#11110F` |
| Surface | `#1B1A17` |
| Elevated surface | `#24221D` |
| Primary text | `#E8DDC7` |
| Secondary text | `#B7A98C` |
| Accent — sand | `#C2A46D` |
| Accent — rust | `#A35E3B` |
| Success | `#7C8F57` |
| Warning | `#C89B3C` |
| Error | `#B45A4E` |

**Typography:** clean, geometric, slightly industrial sans with strong small-size
readability; uppercase *sparingly* for headings/metadata with light letter-spacing; highly
legible numerals (tabular). Avoid ultra-futuristic gimmick fonts (hurt readability).

**Imagery & icons:** echo desert atmosphere through imagery, not decoration; thin-line
icons, consistent stroke weight; simple symbols for server / network / power / storage /
warnings / sync; avoid glossy gradients (or keep extremely subtle); technical, minimal
pictograms.

**Components:** dense-but-spaced cards for server state/actions; status pills
(health, online/offline, version); monospace / tabular numerals for IPs, ports, timestamps,
metrics; soft-to-no shadows (choose "terminal" vs "premium dashboard" feel); low-chroma
buttons by default with accent reserved for primary actions; destructive actions clearly
separated and never color-only.

**Summary recommendation:** a "dark sand UI" — charcoal surfaces, warm text, restrained
gold accents, crisp line icons — Dune-inspired without hurting readability for an ops tool.

**Sources cited by Perplexity:** official media (duneawakening.com, Funcom PR/logo/screenshot
kits), design write-ups (stevenwebbdesign.com/dune-awakening, andiswart.com), and community
UI threads. Researcher flagged no official font pack was publicly exposed.

---

## To revisit at discussion time
- Map the palette onto Radix Themes tokens (accent + gray scale + appearance) and check
  contrast (WCAG AA for body text).
- Decide "terminal" vs "premium dashboard" shadow/elevation feel.
- Icon set choice (consistent thin-line family) + which states get pills.
- Whether/where imagery (dunes/haze) is tasteful vs. noise in an ops tool.

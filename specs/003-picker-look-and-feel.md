---
id: "003"
title: atref picker look & feel — console/monospace aesthetic and polish
status: complete
blocked_by: []
blocks: []
---

# atref Picker Look & Feel

## Overview

Spec 001's picker works but wears default egui styling — proportional font,
flat list, no context. This spec gives it a deliberate **console / monospace
aesthetic** and general polish so it reads like a tool, not a prototype. It
changes only presentation: filtering, insertion, keybindings, tray, and config
are untouched. Match-position highlighting (Roadmap #11) is explicitly
deferred.

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified — the counter math via `cargo test`, and the visual + regression
> ACs through the human-in-the-loop pass below. Build-only verification is
> insufficient. The agent must iterate until verification passes.

## Goals

- A cohesive console/terminal aesthetic: monospace text, fixed-height rows, a
  compact dark frame.
- Clear visual hierarchy: query line, result rows (basename prominent, parent
  dim), an unmistakable selected row.
- At-a-glance context: a `matches/total` counter and a footer of key hints.
- Zero behavior change from specs 001/002.

## Requirements

### Functional Requirements

- **FR1 — Monospace throughout.** All picker text — query input, result rows,
  counter, footer — uses a fixed-width font.
- **FR2 — Console visual style.** A compact, dark, terminal-like frame:
  consistent padding, a subtle border/background, single-line fixed-height
  rows.
- **FR3 — Result row layout.** Each row shows the basename in the primary text
  color and the parent path (relative to its root) in a dimmed color, on one
  line. The selected row is rendered unmistakably — a leading caret (e.g. `>`)
  plus a highlighted/reverse background.
- **FR4 — Match counter.** A header/corner element shows `matches/total` (e.g.
  `8/1,204`) reflecting the current filter, updating per keystroke.
- **FR5 — Footer key hints.** A footer shows the keys that exist today:
  `enter insert · esc cancel · ↑↓ move`. (Format-cycling hints are added when
  Roadmap #14 lands — not here.)
- **FR6 — Framing & placement.** The window is sized for the query line + up to
  10 result rows + the footer, and remains borderless, always-on-top, and
  cursor-anchored (spec 001 FR7).
- **FR7 — Behavior unchanged.** Filtering, selection, `Enter` insertion, `Esc`,
  arrow navigation, the empty-index placeholder, tray, and config behave
  exactly as in specs 001/002.

### Non-Functional Requirements

- **NFR1 — Render cost.** The restyle adds no perceptible per-keystroke cost;
  filter latency stays within the spec 001 budget.
- **NFR2 — Emoji legibility.** Emoji-led filenames (`📦 atref.md`,
  `🏠 Family.md`) render legibly (monochrome per spec 001) and do not break row
  alignment enough to hide the filename text.

### Technical Constraints

- **TC1 — No framework change.** Stay on `eframe`/`egui`; emoji remain
  monochrome (accepted, Roadmap #15). The monospace face may be egui's built-in
  monospace family or a bundled console font — implementer's choice.
- **TC2 — Platform.** Windows 10 (19041+) / 11, x64 — inherited from spec 001.
- **TC3 — No new behavior or runtime dependencies** beyond an optional bundled
  font file.

## Pre-requisites (Human Required)

- [x] Spec 001 complete (it is).

## Implementation Tasks

- [x] Set a monospace font family for all picker text (input, rows, counter,
      footer) (FR1).
- [x] Apply a compact dark/console theme — padding, background, border,
      fixed-height rows, and a caret + highlight for the selected row
      (FR2, FR3).
- [x] Add the `matches/total` counter (FR4).
- [x] Add the footer key-hint line using only currently-bound keys (FR5).
- [x] Size the window to query + 10 rows + footer; keep it borderless,
      always-on-top, cursor-anchored (FR6).
- [x] Confirm behavior parity with specs 001/002 (FR7).

## Acceptance Criteria

### Visual

- [x] **AC1**: All picker text renders in a fixed-width font.
- [x] **AC2**: The picker has a cohesive dark/console look — clearly distinct
      from default egui — with consistent padding, fixed-height rows, and a
      window sized to the query + up to 10 rows + footer.
- [x] **AC3**: Each row shows the basename prominently and the parent path
      dimmed; the selected row is unmistakable (caret + highlight).
- [x] **AC4**: The counter shows the correct `matches/total` and updates as you
      type.
- [x] **AC5**: The footer shows `enter`, `esc`, and `↑↓` hints.
- [x] **AC6**: Emoji-led filenames (`📦 atref.md`) render legibly and keep the
      filename text visible (no alignment breakage that hides it).

### Regression

- [x] **AC7**: Filtering, `Enter` insertion, `Esc`, arrow navigation, the
      empty-index placeholder, tray, and config all behave exactly as before.

### Validation methods

Per `ai-docs/testable-architecture.md` — visual ACs use a one-time snapshot
baseline approval, then automated image-diff on every run.

| AC | Method |
|---|---|
| AC1 (monospace) | `kittest-snapshot` |
| AC2 (console look + sizing) | `kittest-snapshot` |
| AC3 (row layout + selection) | `kittest-snapshot` |
| AC4 (counter) | `unit` + `kittest-input` |
| AC5 (footer) | `kittest-snapshot` |
| AC6 (emoji legibility) | `kittest-snapshot` (one-time human approval of baseline) |
| AC7 (regression / behavior parity) | `unit` + `kittest-input` |

## Testing Approach

### Validation Steps

1. **Build / lint / format:** `cargo build --release`, `cargo clippy
   --all-targets -- -D warnings`, `cargo fmt --check` — clean.
2. **Counter test:** `cargo test` asserts the displayed `matches/total` equals
   the result count and index size for representative queries (AC4).
3. **Visual + regression (human pass):** open the picker and verify AC1–AC3,
   AC5, AC6, then confirm AC7 parity.

### Test Cases

| Input | Expected |
|-------|----------|
| Open picker, empty query, index of 1,204 files | counter reads `≤10/1,204`; up to 10 rows |
| Type `atref` | counter updates to matches/total; rows refilter |
| Select a row | caret + highlight on that row only |
| Row for `📦 atref.md` | emoji + `atref.md` both legible on one line |
| `Enter` on a row (Notepad focused) | inserts `@"…\\📦 atref.md"` (unchanged from 001) |

### Human-in-the-Loop Testing Protocol

This spec is inherently visual, so most ACs need eyes:

1. **Agent:** build the release binary; run the counter test and all automated
   checks.
2. **Agent:** pause and ask JJ to open the picker and judge AC1–AC3, AC5, AC6,
   and confirm AC7 (behavior parity).
3. **JJ:** reports pass/fail per AC, with any "make it look like X" notes.
4. **Agent:** iterate on the styling until JJ signs off on every AC.

## Usage Examples

Target look (current keybindings only):

```text
┌ atref ─────────────────────────────────────────────── 8/1,204 ┐
│ > atref                                                        │
├────────────────────────────────────────────────────────────────┤
│ > 📦 atref.md                              (root)              │
│   001-windows-mvp.md                       atref/specs/        │
│   002-result-quality.md                    atref/specs/        │
│   AGENTS.md                                atref/              │
├────────────────────────────────────────────────────────────────┤
│  enter insert   esc cancel   ↑↓ move                           │
└────────────────────────────────────────────────────────────────┘
```

## Delivered (v0.3.0)

Shipped the console/monospace look behind a testable `picker::render` seam with
`egui_kittest` image-snapshot baselines (AC1–AC3, AC5, AC6) + a `counter_text`
unit test (AC4). Deviations / additions from the original draft, agreed during
testing:

- The selected row is a **full-width teal fill** (the tray-icon teal) with
  high-contrast text, rather than a `>` caret + reverse bar — a clearer
  "unmistakable selection" (AC3 intent).
- Footer reads `enter insert · esc cancel · ↑↓ move · click outside to close`.
- Added beyond the draft: a scrolling result area (cap raised 10 → 50) with
  arrow-nav **clamping** + scroll-into-view; **hide on focus-loss** (click
  outside); a **close ✕**; the dim per-row **source folder** (spec 002, #30).

> The monospace/console aesthetic is **superseded by spec 004** (Raycast /
> PowerToys launcher look — proportional font, rounded window + shadow, airier
> layout). The `render` seam + `egui_kittest` snapshot harness carry forward
> unchanged; spec 004 restyles and regenerates the baselines.

## Out of Scope

- Match-position highlighting (Roadmap #11) — deferred.
- Full-color emoji (Roadmap #15) — egui renders monochrome; not changing here.
- Frecency / recent badges (Roadmap #24).
- Format-cycling key hints (Roadmap #14) — added when that feature lands.
- Any indexing / ranking / behavior change — that is spec 002.
- Settings-window styling (Roadmap #16).

## References

- Project node: `D:\jfuchs\dev\second-brain\📦 atref.md` — Roadmap #12, #13;
  the "Filter UX" picker mockup informs the target aesthetic.
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`.
- Builds on: `001-windows-mvp.md` (complete); layers visually over
  `002-result-quality.md`.

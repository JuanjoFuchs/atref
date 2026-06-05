---
id: "004"
title: atref launcher look — Raycast / PowerToys aesthetic (font, rounded window, airy layout)
status: complete
blocked_by: []
blocks: []
---

# atref Launcher Look

## Overview

Spec 003 gave the picker a console/monospace look behind a snapshot-testable
`picker::render` seam. After living with it, JJ wants the picker to read like a
modern launcher — **Raycast** / **PowerToys Command Palette** — which he finds
prettier. This spec re-skins the picker to that aesthetic: a proportional font,
a rounded window with a drop shadow, an airier layout, a subtle rounded
selected-row highlight, and a chip-style footer. It reuses the spec-003 render
seam and `egui_kittest` snapshot harness unchanged — only the styling and
window chrome change, and the baselines are regenerated.

**Two big Raycast elements are deliberately deferred** (JJ's call): per-row
**icons** and the translucent **acrylic/mica backdrop blur**. This spec gets the
shape, font, and chrome right on a solid window; icons and blur are later passes
(Roadmap).

> **Completion rule:** Not complete until the counter/behavior ACs pass via
> `cargo test` and the visual ACs pass via regenerated `egui_kittest` baselines
> with a one-time human approval of the look (incl. the rounded window + shadow
> on the real desktop). Build-only verification is insufficient.

## Goals

- A modern-launcher aesthetic: proportional font, rounded window + shadow,
  generous spacing, understated selection.
- Reuse the spec-003 `render` seam + snapshot harness; this is a re-skin, not a
  rewrite.
- Zero behavior change from specs 001/002/003.
- Stay cross-platform-friendly (bundle the font; no OS-specific drawing).

## Requirements

### Functional Requirements

- **FR1 — Proportional font.** All picker text uses a clean proportional
  sans-serif (bundled, not system-dependent), replacing the monospace face.
- **FR2 — Rounded window + drop shadow.** The picker renders as a rounded-corner
  panel with a soft drop shadow; the area outside the rounded rect is
  transparent (the OS window is transparent, the panel is drawn). No square
  borderless edges.
- **FR3 — Airy layout.** Launcher proportions: a larger window, generous inner
  padding, taller rows with comfortable vertical rhythm, and a prominent query
  line — closer to Raycast/PowerToys than to a terminal.
- **FR4 — Subtle selected row.** The selected row is an understated rounded
  highlight (a soft dark fill, not a bright full-bleed color), clearly the
  selection but calm. Hover is similarly subtle. (Supersedes spec 003's
  full-width teal row.)
- **FR5 — Chip-style footer / action bar.** The footer reads like a launcher
  action bar — action labels with key "chips" (e.g. `↵ insert`, `esc close`,
  `↑↓ move`) — visually distinct from the result list.
- **FR6 — Counter + close retained.** The `matches / total` counter and a close
  affordance remain, restyled to fit the new look.
- **FR7 — Behavior unchanged.** Filtering, arrow navigation (clamping +
  scroll-into-view), `Enter` insertion, `Esc`, hide-on-focus-loss, the scrolling
  result list, the empty/no-match placeholders, tray, and config behave exactly
  as in specs 001–003.

### Non-Functional Requirements

- **NFR1 — Render cost.** No perceptible per-keystroke cost added; filter
  latency stays within the spec 001 budget.
- **NFR2 — Emoji legibility.** Emoji-led filenames stay legible (monochrome per
  spec 001) and aligned.
- **NFR3 — Transparency safety.** A transparent window must not break the
  off-screen-park show/hide (TC7), cursor-anchored placement + monitor clamping,
  or hide-on-focus-loss. The picker must never leave a transparent/ghost
  artifact on screen when hidden.

### Technical Constraints

- **TC1 — No framework change.** Stay on `eframe`/`egui`. Rounding + shadow via
  an `egui::Frame` (corner radius + `Shadow`) on a transparent viewport
  (`ViewportBuilder::with_transparent(true)`), not OS-specific drawing.
- **TC2 — Platform.** Windows 10 (19041+) / 11, x64.
- **TC3 — Bundled font.** Bundle **Inter** (SIL Open Font License, redistributable)
  via `egui::FontDefinitions` + `ctx.set_fonts`; record the license in-repo. No
  reliance on a system font (keeps snapshots deterministic + ports cleanly).
- **TC4 — Explicitly deferred.** No per-row **icons** and no **acrylic/mica blur**
  in this spec (separate later passes). A teal accent (the brand/tray color) may
  remain as a small accent (e.g. title or selection ring).

## Key Decisions

- **Re-skin, not rewrite.** Keep `picker::render` + `egui_kittest` snapshots from
  spec 003; change `install_theme` (font + visuals + spacing) and the panel
  framing (rounded `Frame` + shadow), then regenerate baselines.
- **Bundle Inter over system fonts.** Deterministic snapshots + cross-platform;
  Segoe UI can't be redistributed and ties us to Windows.
- **Transparent viewport + drawn rounded panel** for rounded corners + shadow —
  pure egui, no DWM. Acrylic/mica blur is a later, Windows-specific pass.
- **Understated selection** replaces spec 003's bright teal full-row fill, per
  the Raycast reference.

## Pre-requisites (Human Required)

- [x] Spec 003 complete (it is).

## Implementation Tasks

- [x] Bundle Inter; load it via `FontDefinitions` as the default proportional
      family; drop the monospace remap (FR1, TC3).
- [x] Render the picker inside a rounded `egui::Frame` with a `Shadow`; make the
      viewport transparent (`with_transparent(true)`); confirm hidden/off-screen
      parking leaves no artifact (FR2, NFR3, TC1).
- [x] Rework spacing/sizing: larger window, padding, row height, query
      prominence (FR3).
- [x] Restyle selected row + hover as a subtle rounded highlight (FR4).
- [x] Restyle the footer as a chip/action bar (FR5).
- [x] Restyle the counter + close to fit (FR6).
- [x] Confirm behavior parity (FR7); regenerate snapshot baselines.

## Acceptance Criteria

### Visual

- [x] **AC1**: All picker text renders in the bundled proportional font (no
      monospace).
- [x] **AC2**: The picker is a rounded-corner panel with a visible drop shadow;
      no square window edges; nothing is drawn outside the rounded rect.
- [x] **AC3**: The layout reads as a launcher — larger window, generous padding,
      comfortable row height — clearly distinct from the spec-003 console look.
- [x] **AC4**: The selected row is a subtle rounded highlight (not a bright
      full-bleed fill); hover is similarly subtle.
- [x] **AC5**: The footer is a chip/action bar showing `↵ insert`, `esc close`,
      `↑↓ move`.
- [x] **AC6**: The `matches / total` counter and a close affordance are present
      and restyled.
- [x] **AC7**: Emoji-led filenames stay legible and aligned.

### Regression

- [x] **AC8**: Filtering, arrow nav (clamp + scroll-into-view), `Enter`
      insertion, `Esc`, hide-on-focus-loss, scrolling, placeholders, tray, and
      config all behave exactly as in specs 001–003. Hiding leaves no on-screen
      artifact (NFR3).

### Validation methods

Per `ai-docs/testable-architecture.md` — visual ACs regenerate the snapshot
baselines (one-time human approval), behavior is unit/kittest-tested.

| AC | Method |
|---|---|
| AC1 (font) | `kittest-snapshot` |
| AC2 (rounded + shadow) | `kittest-snapshot` + one-time human approval (translucency/shadow on the real desktop) |
| AC3 (airy layout) | `kittest-snapshot` |
| AC4 (subtle selection + hover) | `kittest-snapshot` |
| AC5 (chip footer) | `kittest-snapshot` |
| AC6 (counter + close) | `kittest-snapshot` + the `counter_text` unit test |
| AC7 (emoji) | `kittest-snapshot` |
| AC8 (regression / no-artifact) | `unit` + `kittest-input` + a short `manual` smoke |

## Testing Approach

### Validation Steps

1. **Build / lint / format:** `cargo build --release`, `cargo clippy
   --all-targets -- -D warnings`, `cargo fmt --check` — clean.
2. **Counter + behavior tests:** `cargo test` (existing suite stays green; the
   `counter_text` unit test covers AC6).
3. **Snapshots:** regenerate baselines (`UPDATE_SNAPSHOTS=1`), commit, then
   `cargo test` confirms diff-match (AC1, AC3–AC7).
4. **Human pass:** open the picker; approve the rounded window + shadow + font +
   spacing on the real desktop (AC2), and confirm AC8 parity incl. no
   transparent/ghost artifact when hidden.

### Test Cases

| Input | Expected |
|-------|----------|
| Open picker | rounded panel + shadow; proportional font; airy spacing |
| Type a query | counter updates; rows refilter (unchanged) |
| Select a row | subtle rounded highlight on that row only |
| Hover a row | subtle highlight, calm |
| `Esc` / click outside | picker hides; no leftover transparent artifact |
| `Enter` on a row (Notepad focused) | inserts `@"…"` (unchanged from 001) |

### Human-in-the-Loop Testing Protocol

1. **Agent:** build; regenerate baselines; run all automated checks.
2. **Agent:** ask JJ to open the picker, approve the look (AC2 especially —
   shadow/rounding/font on the desktop), and confirm AC8 parity + no artifact.
3. **JJ:** pass/fail per AC with any "make it look more like X" notes.
4. **Agent:** iterate until sign-off; then commit + bump version.

## Delivered (v0.4.0)

Shipped the launcher look behind the same `picker::render` seam + `egui_kittest`
snapshots. Key as-built deviations, found during testing:

- **Rounded corners via OS region clip, not transparency.** eframe window
  transparency (`with_transparent(true)` + transparent `clear_color`) rendered an
  **opaque black** background on Windows under *both* the glow and wgpu backends.
  So the window is **opaque** (panel-colored) and its corners are rounded by a
  Win32 `SetWindowRgn` round-rect region (`round_window` in `main.rs`, applied on
  first focus). Consequence: **no soft drop shadow** (FR2's shadow needs
  transparency) and corners are hard-clipped (not anti-aliased). Reverted the
  wgpu experiment back to glow.
- **Font:** egui's bundled proportional face (Ubuntu-Light), not Inter — Inter
  bundling deferred (avoids shipping a TTF; the default is deterministic +
  cross-platform). A monospace fallback is appended so glyphs the sans lacks
  (`↑↓`) still render.
- **Footer:** chip-style buttons — `enter insert` / `esc close` are clickable
  (insert the selection / close), `↑↓ move` is a disabled chip; text selection is
  globally disabled so labels show the arrow cursor, not an I-beam. Close is a
  larger `×` button.

> A true soft shadow + anti-aliased corners would need a different windowing
> approach (Win32 layered window / DWM) — a possible later pass. Per-row icons
> and acrylic blur remain deferred (#32 and later).

## Out of Scope

- **Per-row icons** (file-type or shell icons) — a later pass (Roadmap; the most
  effortful Raycast element).
- **Acrylic / mica backdrop blur** — a later, Windows-specific pass.
- Right-side **preview / metadata pane** (Roadmap #27).
- Command-palette **actions** (PowerToys-style `>`/`=` modes) — different product
  surface; atref is a file picker.
- Match-position highlighting (#11), frecency (#24), format cycling (#14).
- Any indexing / ranking / behavior change.

## References

- Project node: `D:\jfuchs\dev\second-brain\📦 atref.md` — Roadmap #31.
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`.
- Builds on: `003-picker-look-and-feel.md` (complete) — reuses its
  `picker::render` seam + `egui_kittest` snapshots.
- Visual references: Raycast file search; PowerToys Command Palette.
- Font: [Inter](https://rsms.me/inter/) (SIL Open Font License 1.1).

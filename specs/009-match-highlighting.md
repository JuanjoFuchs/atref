---
id: "009"
title: Match-position highlighting
status: complete
blocked_by: []
blocks: []
---

# Match-position Highlighting

## Overview

When the picker filters results, emphasize the exact characters in each row that
matched the current query, so the user can see *why* a result ranks where it does.
This is roadmap capability **#11** and one of the "top 10 filter-UX patterns to
copy" recorded in the atref project node — hiding matched-char highlights is listed
there as an explicit trust-destroying anti-pattern.

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through the validation method each names (unit / live-gui).
> Build-only verification is insufficient. The agent must iterate until
> verification passes.

> **Delivered (2026-06-09):** highlights ride the ranker's own nucleo indices
> (computed for returned rows only), exchanged as per-row code-point positions
> and rendered as accent runs; an `aria-label` keeps the full filename readable
> where highlight runs fragment the accessibility tree. Verified by 8 unit tests
> and the `rich_rows_show_metrics_and_thumbnails` live-GUI gate (screenshots in
> `target/e2e-artifacts/`).

## Goals

- Show which characters of each result matched the active query.
- Update highlights live, per keystroke, with no perceptible lag.
- Highlights must reflect the **same** match the ranker used to score/order the row —
  never a separately re-derived match.

## Requirements

### Functional Requirements

- **FR1**: Each result row visually emphasizes the characters that the fuzzy matcher
  matched for the current query, in both the basename and the location (path) text.
- **FR2**: Highlighting recomputes on every query change and clears entirely on an
  empty query (empty query = frecency listing, no match to show).
- **FR3**: The highlighted character positions are the ranker's own match indices, so
  what's emphasized is exactly what was scored — no second, divergent matching pass.

### Non-Functional Requirements

- **NFR1**: Stays within the per-keystroke budget the project node sets (< 8 ms
  end-to-end, matcher < 4 ms); highlight computation must not blow that. Index
  extraction therefore runs only for the rows actually returned (≤ 50), not for
  every entry scored.

### Technical Constraints

- **TC1**: The matcher in use (nucleo) can return match indices alongside the score
  (`Pattern::indices`, same score as `Pattern::score`); use that capability rather
  than re-implementing matching. nucleo documents the returned indices as possibly
  unsorted and duplicated — they must be sorted and deduplicated before use.
- **TC2**: Matching runs over the root-relative path, but the UI displays two derived
  strings (basename, and `root-folder/parent` location with `\` shown as `/`).
  Match positions must be mapped onto those displayed strings; positions landing on
  the boundary separator (shown in neither string) are dropped.
- **TC3**: The UI is a WebView2/HTML frontend; highlighting is presentation markup
  around matched characters, built via DOM nodes (no innerHTML injection — file names
  may contain `<`, `&`, etc.). Positions are exchanged as **Unicode code-point
  indices** into the displayed strings (`name_hl`, `loc_hl` arrays on each search
  result row), which JS consumes via code-point iteration (`Array.from`). Must remain
  correct under the existing smart-case / CamelHumps matching (matched chars may be
  non-contiguous).

## Implementation Tasks

- [x] Extract match indices from the search layer for returned rows only, reusing the
  same pattern construction the ranker uses (TC1, NFR1).
- [x] Map rel-relative indices onto the displayed basename/location strings (TC2).
- [x] Extend the `search_files` response rows with `name_hl` / `loc_hl` code-point
  index arrays; empty on empty query (FR2).
- [x] Render highlights in the picker rows as accent-styled spans built from DOM
  nodes (TC3).
- [x] Unit tests for index extraction and display mapping; live-GUI screenshot pass.

## Acceptance Criteria

### Truthful highlights
- [x] **AC1**: For a non-empty query, every returned row carries match positions
  produced by the same nucleo pattern invocation family that scored it, and the
  indices-producing call yields the same score as the scoring call (FR3/TC1). `unit`
- [x] **AC2**: Positions map correctly onto the displayed strings: nested file
  (highlights split across name and location), root-level file (name only, location
  is the bare root folder name), query char hitting the boundary separator (dropped),
  non-ASCII filename (code-point positions, not bytes) (FR1/TC2). `unit`
- [x] **AC3**: A CamelHumps-style query (e.g. initials of a kebab-case name) yields
  non-contiguous highlight positions matching the chars nucleo actually matched (TC3).
  `unit`

### Lifecycle
- [x] **AC4**: Empty query returns rows with empty `name_hl`/`loc_hl` and the UI
  renders them with no highlight markup (FR2). `unit` for the payload; `live-gui`
  for the render.

### Performance & presentation
- [x] **AC5**: Index extraction is performed only for returned rows (≤ MAX_RESULTS),
  and a timing smoke over a synthetic multi-thousand-entry corpus stays within a
  debug-tolerant bound (NFR1). `unit`
- [x] **AC6**: In the live picker, a query visibly emphasizes its matched characters
  in the accent style across name and location text. `live-gui` (screenshot per
  `ai-docs/agentic-gui-testing.md`)

## Testing Approach

### Validation Steps
1. `cargo test` — unit tests for extraction, mapping, empty-query, timing smoke.
2. `cargo test --test e2e -- --ignored --nocapture --test-threads=1` — live picker
   screenshot showing highlighted chars (desktop must be free; serial — the gates
   share the desktop and chord).
3. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`.

### Test Cases
| Input | Expected Output |
|-------|-----------------|
| Query `spec` over `specs\009-match-highlighting.md` | Positions for `s`,`p`,`e`,`c` present; score equals the ranking score |
| Query hitting only the basename of `docs\demo.mp4` | All positions in `name_hl`, none in `loc_hl` |
| Query `dm` over `docs\demo.mp4` (chars in both segments) | Positions split across `loc_hl` (offset past root folder name + `/`) and `name_hl` |
| Root-level file, query matches name | `loc_hl` empty |
| Empty query | `name_hl`/`loc_hl` empty for every row |
| Non-ASCII name (e.g. `nota-café.md`), query `café` | Code-point indices align with `Array.from` positions |

## Out of Scope

- Highlighting inside a preview pane (roadmap #27).
- Regex or whole-string highlighting.
- User-configurable highlight styling/theming.

## References

- Roadmap **#11** (atref project node, second brain) — "Match-position highlighting",
  Wave 2; and the project node's "Top 10 patterns to copy" / "Anti-patterns to avoid".
- Existing matcher integration: spec 002 (result quality) and the `search` ranking
  seam already unit-tested there.
- GUI validation lane: `ai-docs/agentic-gui-testing.md`; seams:
  `ai-docs/testable-architecture.md`.

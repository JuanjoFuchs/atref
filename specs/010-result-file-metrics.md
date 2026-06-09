---
id: "010"
title: Per-result file metrics (size, lines, tokens)
status: complete
blocked_by: []
blocks: []
---

# Per-result File Metrics — size · lines · ~tokens

## Overview

Surface, in the picker, how "big" a file is along three axes: byte size, line count,
and an estimated token count. The token estimate is the headline value — atref is the
`@`-picker for agent workflows, and the question it uniquely answers is *"how much of
my context window will inserting this reference cost?"* This is roadmap capability
**#37**. Metrics render inline on each result row, right-aligned in the same dim
style as the location text (JJ's placement decision, 2026-06-09).

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through the validation method each names (unit / integration /
> live-gui). Build-only verification is insufficient. The agent must iterate
> until verification passes.

> **Delivered (2026-06-09):** size/mtime now ride the index (the store already
> persisted both — no schema bump), an async enrichment command computes
> lines + tiktoken-o200k ~tokens off-thread behind the shared cloud-placeholder
> guard and a bounded `(path, mtime)` cache, and rows show `size · ln · ~tok`
> right-aligned, filling in for visible rows ~150 ms after input settles.
> Verified by unit + `attrib +O` integration tests and the live-GUI gate
> (`"16 B · 1 ln · ~4 tok"` read back via UIA). Binary grew ~7 MB for the
> embedded o200k ranks (approved). The manual feel-check sliver: confirm in
> daily use that typing stays instant in huge folders.

## Goals

- Show file **size** on every result row cheaply (no file-content read).
- Show **line count** and an **estimated token count** on visible rows, lazily.
- Never trigger a cloud download or stall the picker to compute any of this.

## Requirements

### Functional Requirements

- **FR1**: A result row's byte size is available for display without reading file
  contents — it flows from the filesystem metadata the index already captures — and
  renders immediately with the row.
- **FR2**: Visible rows additionally show their line count and an estimated token
  count, right-aligned with the size in the row's metrics text (e.g.
  `3.2 KB · 95 ln · ~1.1k tok`). The token figure is presented as an estimate
  (`~` prefix).
- **FR3**: Line/token computation is **lazy**: requested only for rows actually
  visible in the viewport, only after input settles (~150 ms idle), and cached —
  never eagerly across the whole result list or index.
- **FR4**: Results that are cloud-only / offline placeholders show **size only**;
  their contents are never read, so lines/tokens are omitted and no hydration is
  triggered.
- **FR5**: Binary / non-text files, and files above the text-metrics size cap, show
  size only (no meaningful line/token count).

### Non-Functional Requirements

- **NFR1**: Metric computation never blocks the picker's UI thread or regresses the
  per-keystroke budget (project node performance table); typing while metrics resolve
  must stay instant.
- **NFR2**: Computed metrics are cached and reused while the underlying file is
  unchanged; the cache is bounded.

### Technical Constraints

- **TC1 (cloud-safe — architecture-level, do not skip)**: Reading a file's *contents*
  hydrates a cloud placeholder (OneDrive/Dropbox), as proven on 2026-06-09 when
  `git_aware`'s `.gitignore` reads pulled files down. Detect placeholder/offline files
  via the Windows file attributes `FILE_ATTRIBUTE_OFFLINE` (`0x1000`) and
  `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` (`0x400000`) — readable from std metadata,
  no new Win32 bindings — and skip their content reads. This is a shared guard:
  spec 011 (thumbnails) reuses it.
- **TC2 (tokenizer — resolved 2026-06-09)**: Use `tiktoken-rs` with the `o200k_base`
  encoding as the closest public approximation (JJ approved the dependency and binary
  growth). Initialize the encoder lazily off the UI thread. The UI labels the figure
  an estimate either way.
- **TC3**: Caching is keyed by path + last-modified time so edits invalidate stale
  metrics.
- **TC4 (IPC contract)**: The frontend requests enrichment per file and receives
  `{ size: number, lines: number|null, tokens: number|null }` (spec 011 extends this
  with `thumb`). `null` means "size only" (cloud-only, binary, or over-cap). Search
  result rows carry `size` directly so it renders without a round-trip.

## Implementation Tasks

- [x] Carry size + mtime on indexed entries (the store already persists both; no
  schema bump) and expose size on search result rows.
- [x] Shared cloud-placeholder guard over std file attributes (TC1).
- [x] Enrichment logic behind a testable seam: line counting, binary sniff, size cap,
  tiktoken `o200k_base` token estimate, lazily initialized (TC2).
- [x] Bounded enrichment cache keyed `(path, mtime)` (TC3, NFR2).
- [x] Async enrichment command doing file IO off the UI thread (NFR1, TC4).
- [x] UI: right-aligned per-row metrics text; size immediate, lines/tokens fill in
  for visible rows after input settles; formatters for size and token counts (FR2/FR3).
- [x] Unit/integration tests per the ACs; live-GUI pass.

## Acceptance Criteria

### Size without content reads
- [x] **AC1**: Search result rows carry byte size sourced from index/store metadata —
  no file-content read on the search path (FR1). `unit` (entry/store round-trip
  preserves size) + `live-gui` (size visible in rows).

### Lazy text metrics
- [x] **AC2**: Enriching a text file yields its correct line count (incl. CRLF and
  no-trailing-newline cases) and a token estimate > 0 that grows with content length
  (FR2/TC2). `unit`
- [x] **AC3**: Enrichment is requested per-file on demand and cached: the same
  `(path, mtime)` is computed once; bumping mtime invalidates; the cache stays within
  its bound (FR3/TC3/NFR2). `unit`

### Size-only fallbacks
- [x] **AC4**: A file with `FILE_ATTRIBUTE_OFFLINE` set returns size with
  `lines`/`tokens` null and its contents are never opened (FR4/TC1). `integration`
  (temp file + `attrib +O`)
- [x] **AC5**: A binary file (NUL byte in its first 8 KiB) returns size only (FR5).
  `unit`
- [x] **AC6**: A file larger than the text-metrics cap returns size only without
  reading its contents (FR5). `unit`

### Presentation & responsiveness
- [x] **AC7**: In the live picker, visible rows show `size · N ln · ~N tok`
  right-aligned in the dim style; the token figure carries the `~` estimate marker;
  typing stays instant while metrics resolve (FR2/NFR1). `live-gui` (UIA text for a
  known fixture + screenshot) + `manual` (feel check — irreducible: perceived input
  latency).

## Testing Approach

### Validation Steps
1. `cargo test` — enrichment logic, cache, guard unit/integration tests; full suite
   for regressions.
2. `cargo test --test e2e -- --ignored --nocapture --test-threads=1` — live picker
   shows metrics for a fixture file (desktop must be free; serial).
3. Manual smoke against a OneDrive cloud-only file: size shows, no download starts
   (THE regression this spec must not cause).

### Test Cases
| Input | Expected Output |
|-------|-----------------|
| 3-line UTF-8 file, no trailing newline | `lines = 3`, `tokens > 0` |
| Same file re-enriched, mtime unchanged | served from cache (computed once) |
| Same path, mtime bumped | recomputed |
| File with `attrib +O` (offline) | size only; no content read |
| File with `0x00` in first 8 KiB | size only |
| File over the text-metrics cap | size only; no content read |
| Empty file | `lines = 0`, `tokens = 0` |

## Out of Scope

- Exact Claude tokenization (no public offline tokenizer exists).
- Line/token counts for rows outside the viewport.
- Configurable metric display / column toggles / cap tuning.
- A full preview pane (#27) — this is a metrics line, not file contents.
- Skipping cloud placeholders during *indexing* (they remain valid, referenceable
  results; only content reads are guarded).

## References

- Roadmap **#37** (atref project node, second brain), and the 2026-06-09 OneDrive
  hydration lesson recorded there (the `git_aware` `.gitignore` read incident).
- Sibling enrichment feature sharing the guard + pipeline: thumbnails (#38 /
  spec 011). Highlighting (#11 / spec 009) shares the row-payload seam.
- Seams: `ai-docs/testable-architecture.md`; GUI lane: `ai-docs/agentic-gui-testing.md`.

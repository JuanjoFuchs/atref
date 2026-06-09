---
id: "011"
title: Thumbnails for media file types
status: complete
blocked_by: ["010"]
blocks: []
---

# Thumbnails for Media File Types

## Overview

Show a small inline thumbnail for image results so a picture is recognizable at a
glance instead of being just a filename. This is roadmap capability **#38**, and the
heaviest of the "rich result" cluster (#11 highlight, #37 metrics, #27 preview pane) —
so it ships images first, video later. Thumbnails ride the spec-010 enrichment
pipeline: same lazy visible-row requests, same `(path, mtime)` cache, same
cloud-placeholder guard, rendered at the row's right edge next to the metrics
(JJ's inline-placement decision, 2026-06-09).

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through the validation method each names (unit / integration /
> live-gui). Build-only verification is insufficient. The agent must iterate
> until verification passes.

> **Delivered (2026-06-09):** rasters decode + downscale to a 56 px PNG data
> URI (GIF = first frame), SVG passes through as `data:image/svg+xml` with text
> metrics from the same read; all riding spec 010's enrichment payload, cache,
> caps, and cloud guard. The row's thumbnail carries `alt="thumbnail"` (a11y +
> UIA-assertable). Verified by unit + `attrib +O` integration tests and the
> live-GUI gate screenshot (red fixture thumb at the row edge).

## Goals

- Render a small preview thumbnail for recognized **image** results.
- Generate thumbnails lazily and cheaply; never stall the picker or trigger a cloud
  download.
- Lay groundwork that a later iteration extends to video frame-grabs.

## Requirements

### Functional Requirements

- **FR1**: For recognized image types (`png`, `jpg`/`jpeg`, `gif`, `webp`, `svg`),
  visible result rows show a small thumbnail at the row's right edge. Animated GIFs
  thumbnail as their first frame.
- **FR2**: Thumbnails are produced **lazily** through the same visible-rows-after-
  input-settles enrichment requests as spec 010 — one request per file covers
  metrics and thumbnail; never eager across the whole index or result list.
- **FR3**: A produced thumbnail is **cached** (the spec-010 `(path, mtime)` cache)
  and reused while the file is unchanged.
- **FR4**: Cloud-only / offline placeholder files are **not** decoded — no thumbnail,
  no hydration.
- **FR5**: Non-media results are unaffected (no thumbnail; row layout unchanged
  beyond the image appearing on media rows).

### Non-Functional Requirements

- **NFR1**: Thumbnail work happens off the UI thread and respects the picker's
  instant-feel budget; a slow/large image must not block typing or selection.
- **NFR2**: Large source images are downscaled before display — no multi-MB payloads
  pushed into the view.

### Technical Constraints

- **TC1 (cloud-safe — architecture-level, do not skip)**: Decoding an image reads its
  contents, which hydrates a cloud placeholder. Reuse spec 010's offline-placeholder
  guard (`FILE_ATTRIBUTE_OFFLINE` / `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`) — one
  shared check, not two.
- **TC2 (delivery — resolved 2026-06-09)**: Rasters are decoded and downscaled
  off-thread to a bounded square (56 px — 2× the 28 px row display, for hidpi),
  re-encoded as PNG, and delivered to the WebView as a `data:image/png;base64,…` URI
  in the enrichment payload (`thumb: string|null` alongside spec 010's fields). SVG
  is text the WebView renders natively: passed through as `data:image/svg+xml` up to
  a size cap. No asset-protocol scope is opened.
- **TC3**: Source-size caps guard the decode: oversized rasters and SVGs are skipped
  (no thumbnail, metrics unaffected).
- **TC4**: Type recognition is by file extension — content sniffing would require
  the very read the cloud guard exists to avoid.

## Implementation Tasks

- [x] Promote the `image` crate to a runtime dependency with png/jpeg/gif/webp decode.
- [x] Thumbnail generation behind a pure seam (`image bytes → bounded PNG data URI`),
  plus extension-based type recognition.
- [x] Extend the spec-010 enrichment payload and pipeline with `thumb` (shared guard,
  caps, cache).
- [x] Render the thumbnail at the row's right edge; rows without one are unchanged.
- [x] Unit/integration tests per the ACs; live-GUI screenshot pass.

## Acceptance Criteria

### Generation
- [x] **AC1**: Thumbnailing a larger raster yields a valid PNG data URI whose decoded
  dimensions are ≤ the bound, preserving aspect ratio (NFR2/TC2). `unit`
- [x] **AC2**: A GIF thumbnails from its first frame; png/jpeg/webp decode likewise;
  recognized extensions map to raster/svg kinds and others to none (FR1/TC4). `unit`
- [x] **AC3**: Enriching an image file returns metrics **and** a thumbnail in one
  payload; a non-image file returns `thumb: null`; an oversized raster/SVG returns
  no thumbnail (FR2/FR5/TC3). `unit`
- [x] **AC4**: A cloud-only image (offline attribute) returns no thumbnail and its
  contents are never read (FR4/TC1). `integration` (attrib +O fixture)

### Caching & presentation
- [x] **AC5**: Thumbnails are served from the `(path, mtime)` cache while the file is
  unchanged — one decode per (path, mtime) (FR3). `unit` (shared cache semantics,
  spec 010 AC3) + code-path review that `thumb` rides the same cache entry.
- [x] **AC6**: In the live picker, image fixtures show a thumbnail at the row's right
  edge; non-image rows render exactly as before (FR1/FR5). `live-gui` (screenshot per
  `ai-docs/agentic-gui-testing.md`)

## Testing Approach

### Validation Steps
1. `cargo test` — generation, type recognition, payload, guard tests; full suite for
   regressions.
2. `cargo test --test e2e -- --ignored --nocapture --test-threads=1` — live picker
   over a fixture folder containing images (desktop must be free; serial);
   screenshot shows thumbs.
3. Manual smoke: query an image-heavy folder; typing stays instant while thumbs fill.

### Test Cases
| Input | Expected Output |
|-------|-----------------|
| 200×100 PNG | data URI PNG, ≤ 56 px, 2:1 aspect kept |
| Small GIF (2 frames) | thumbnail of frame 1 |
| `notes.md` | `thumb: null`, metrics as spec 010 |
| SVG under cap | `data:image/svg+xml` URI + text metrics |
| SVG over cap | no thumbnail |
| Raster over decode cap | no thumbnail, size still shown |
| Image with `attrib +O` | no thumbnail, no content read |

## Out of Scope

- **Video thumbnails / frame-grabs** — deferred to a follow-on once the image path
  ships (needs an ffmpeg-class dependency).
- PDF, document, or other non-image previews.
- A full preview pane (#27) or an enlarged/lightbox preview.
- Configurable thumbnail size / on-off toggle.

## References

- Roadmap **#38** (atref project node, second brain).
- Shared pipeline, guard, and cache: spec 010 (file metrics, #37) and the 2026-06-09
  OneDrive hydration lesson.
- Related "rich result" cluster: match highlighting (#11 / spec 009), preview pane
  (#27).
- Seams: `ai-docs/testable-architecture.md`; GUI lane: `ai-docs/agentic-gui-testing.md`.

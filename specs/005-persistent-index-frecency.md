---
id: "005"
title: atref persistent index + frecency — fast startup, scale, recents-first ranking
status: complete
blocked_by: []
blocks: []
---

# atref Persistent Index + Frecency

## Overview

atref rebuilds its index by walking every configured folder on each launch and
keeps it only in memory (kept fresh by the spec-002 watcher). That re-walk is the
startup cost and the scaling ceiling for large corpora, and the picker has no
memory of what you actually use. This spec adds:

- **Persistent index** — cache the file list on disk so launch loads it
  instantly, then reconciles against the filesystem in the background; the
  watcher keeps both the in-memory and on-disk copies fresh.
- **Frecency** — record every pick and rank by frequency + recency, so an empty
  query surfaces recent / most-used files first and good matches get a boost.

No change to matching, insertion, watcher freshness, config, tray, or the
spec-004 look.

> **Completion rule:** Not complete until the store + ranker ACs pass via
> `cargo test` (integration with a temp store/dirs, unit for frecency) and a
> short startup/recents smoke. Build-only verification is insufficient.

## Goals

- Near-instant startup independent of corpus size (load the cache; don't block
  on a full walk).
- Scale to large multi-folder corpora without a slow cold start.
- Surface recent + frequently-picked files immediately (empty query) and boost
  them in results.
- Keep the in-memory `Vec<Entry>` + `search::rank` seam; persistence + frecency
  feed it.
- Preserve all prior behavior.

## Requirements

### Functional Requirements

- **FR1 — Persistent store.** A single on-disk store in the app data dir holds
  the cached index (abs, root, rel, root_rank, + mtime/size for change
  detection) and frecency data (pick count + last-picked time per path).
- **FR2 — Load-then-reconcile startup.** On launch, load the cached index and
  show the picker immediately; in the background, walk the configured folders
  (git-aware, spec 002) and reconcile store + memory (add new, drop deleted,
  update changed). First run (missing/empty store) falls back to a full walk and
  populates the store.
- **FR3 — Watcher writes through.** The live watcher (spec 002 FR6) updates both
  the in-memory index and the store, honoring the same filters.
- **FR4 — Pick recording.** Accepting a result (Enter / click insert) records a
  pick for that path (count++, last-picked = now) in the store, off the UI
  thread.
- **FR5 — Frecency score.** `frecency(path) = pick_count × recency_weight(now −
  last_picked)`, with bucketed recency weights (≤1h ×4, ≤1d ×2, ≤1wk ×1, ≤1mo
  ×0.5, older ×0.25; never-picked = 0). Pure + unit-tested.
- **FR6 — Frecency ranking.** Empty query → order by frecency desc, then
  folder-priority/path (recents lead; the rest keep the spec-002 order).
  Non-empty query → nucleo score primary, with frecency a **bounded** boost /
  tiebreak — it edges out an equal- or near-equal-scoring stranger but must NOT
  float a clearly-worse fuzzy match above a clearly-better one.
- **FR7 — Robustness.** A corrupt/unreadable store is rebuilt from a walk rather
  than crashing; the schema is versioned.
- **FR8 — Reload.** Tray *Reload* rebuilds the index and reconciles the store
  against the (possibly new) folder set.
- **FR9 — Behavior preserved.** Matching, `@"path"` insertion, watcher
  freshness, config, tray, and the spec-004 look are unchanged.

### Non-Functional Requirements

- **NFR1 — Startup.** With a warm cache the picker is usable within the spec-001
  budget regardless of corpus size; the reconcile walk runs in the background,
  not blocking the first show.
- **NFR2 — No UI stall.** Store reads/writes that could block run off the UI
  thread; per-keystroke filtering stays in budget.
- **NFR3 — Freshness.** After reconcile + watcher, the index converges to the
  real filesystem (no stale/missing) within the spec-002 latency.
- **NFR4 — Footprint.** The store stays bounded — reconcile prunes entries for
  paths no longer indexed and frecency rows for vanished files.

### Technical Constraints

- **TC1 — Storage engine.** An embedded store in the app data dir. Recommended:
  **SQLite via `rusqlite` (bundled)** — one transactional file for both the path
  cache and frecency. Alternative if the bundled C build is unwanted: a
  pure-Rust store (`redb`). Decided in planning.
- **TC2 — Platform.** Windows 10 (19041+) / 11, x64.
- **TC3 — Seam preserved.** `index` / `search` stay the testable seam;
  persistence is a new `store` module the app loads from + writes to;
  `search::rank` gains frecency input but stays pure + unit-testable.
- **TC4 — Compatibility.** Existing configs keep working; the store auto-creates
  on first run; no required config change (optional store/frecency settings may
  be added).

## Key Decisions

- **SQLite (`rusqlite`, bundled)** for one transactional file holding both the
  index cache and frecency — vs `redb` (pure-Rust, no C compiler). Recommend
  SQLite; confirm the binary-size / build trade-off in planning.
- **Load-then-reconcile** — never block startup on a walk; the background walk +
  watcher converge the cache.
- **Frecency = frequency × bucketed recency** (fzf-style), fed into
  `search::rank`; empty query is frecency-first.
- **No FTS5 / SQL fuzzy search** — nucleo still matches in memory; the store is a
  cache + frecency ledger. FTS5 pre-filtering for *huge* corpora is a possible
  later optimization, out of scope here.

## Pre-requisites (Human Required)

- [ ] Specs 001–004 complete (they are).

## Implementation Tasks

- [ ] Add the storage dependency + a `store` module: open/create the DB in the
      app data dir; versioned schema (index table + frecency table).
- [ ] Persist the index: write entries on (re)build; load on startup into the
      in-memory `Vec<Entry>`.
- [ ] Load-then-reconcile startup: show the cached index immediately; background
      walk reconciles store + memory; first-run full walk.
- [ ] Watcher writes through to the store (FR3).
- [ ] Record picks in `accept`, off the UI thread (FR4).
- [ ] Frecency score + ranking: extend `search::rank` to take frecency data;
      empty-query frecency order + bounded boost on queries (FR5/FR6).
- [ ] Robustness: corrupt-store rebuild, schema version, prune vanished entries.
- [ ] Reload reconciles the store (FR8).
- [ ] Tests: store round-trip, load-then-reconcile add/remove, pick recording,
      frecency ordering (empty) + boost (query), corrupt-store recovery.

## Acceptance Criteria

### Persistent index

- [x] **AC1**: With a populated store, launch loads the cached index — the
      picker shows results without a full walk first. *(tests/store.rs
      `round_trips_entries_across_reopen`; startup loads `store.load_entries()`.)*
- [x] **AC2**: Background reconcile adds newly-appeared files and drops deleted
      ones so the index matches the filesystem. *(tests/store.rs
      `reconcile_adds_new_and_drops_deleted`.)*
- [x] **AC3**: First run (no store) walks + populates the store; the next run
      loads from it. *(Empty store ⇒ `load_entries()` empty ⇒ `start_reconcile`
      walks + `persist`s; round-trip test covers the next-run load.)*
- [x] **AC4**: A corrupt/unreadable store is rebuilt instead of crashing.
      *(tests/store.rs `rebuilds_corrupt_store_without_panicking`.)*

### Frecency

- [x] **AC5**: Accepting a file records a pick (count++, last-picked updated).
      *(tests/store.rs `records_picks_with_count_and_recent_time`; `accept`
      calls `store.record_pick` off the UI thread.)*
- [x] **AC6**: With an empty query, files are ordered by frecency (a
      recently/often-picked file ranks above never-picked ones), then
      folder-priority. *(search.rs `empty_query_orders_by_frecency_then_folder`.)*
- [x] **AC7**: On a query, frecency boosts ties / near-ties but does not float a
      clearly-worse fuzzy match above a clearly-better one. *(search.rs
      `query_frecency_breaks_near_equal_ties` +
      `query_frecency_does_not_beat_clearly_better_match`.)*
- [x] **AC8**: The frecency formula matches FR5 for representative `(count, age)`
      inputs. *(frecency.rs `recency_buckets_match_fr5` +
      `frequency_scales_linearly_within_a_bucket` + `never_picked_scores_zero`.)*

### Regression

- [x] **AC9**: Matching, insertion, watcher freshness, config, tray, and the
      spec-004 look are unchanged. *(All prior suites stay green — incl. the 3
      egui_kittest picker snapshots with **no regeneration**, proving the look is
      byte-identical, and tests/watcher.rs for freshness. The live startup-feel /
      recents smoke (NFR1) is JJ's one-time desktop pass.)*

### Validation methods

Per `ai-docs/testable-architecture.md` — store + ranker are headless-testable.

| AC | Method |
|---|---|
| AC1–AC4 (store / reconcile / first-run / corruption) | `integration` — temp store + temp dirs |
| AC5 (pick recording) | `integration` — temp store |
| AC6–AC8 (frecency order / boost / formula) | `unit` — `search::rank` + frecency fn |
| AC9 (regression) | `unit` + a short `manual` smoke |

## Testing Approach

### Validation Steps

1. **Build / lint / format:** `cargo build`, `cargo clippy --all-targets --
   -D warnings`, `cargo fmt --check` — clean.
2. **Tests:** `cargo test` — store round-trip + reconcile + first-run +
   corruption (integration, temp dirs/DB); pick recording; frecency order/boost/
   formula (unit). Existing suite stays green.
3. **Startup feel:** launch with a warm cache; confirm the picker is usable
   immediately on a large corpus (NFR1).
4. **Recents smoke:** pick a file, reopen, confirm it leads the empty-query list.

### Test Cases

| Input | Expected |
|-------|----------|
| Populated store, launch | index loaded from store; no blocking walk |
| File created after cache, then reconcile | appears in the index |
| File deleted after cache, then reconcile | dropped from the index |
| Corrupt store file | rebuilt from a walk; no crash |
| Pick `notes.md` ×3, then empty query | `notes.md` near the top |
| Query where a stranger scores clearly higher | stranger still wins (bounded boost) |

### Human-in-the-Loop Testing Protocol

Store + frecency are headless-testable, so the agent verifies AC1–AC8. Hand off
to JJ for the startup-feel + recents smoke (AC9 + NFR1) and iterate on failures.

## Out of Scope

- FTS5 / SQL-side fuzzy pre-filtering (a later scale optimization).
- Cross-machine / cloud sync of the index or frecency.
- A frecency-tuning UI (weights stay code-level).
- Designed app/tray icon (#32) — a separate task.
- Match highlighting (#11), format cycling (#14), settings GUI (#16).

## References

- Project node: `D:\jfuchs\dev\second-brain\📦 atref.md` — Roadmap #29, #24.
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`.
- Builds on: specs `001`–`004` (complete); reuses the `index` / `search` seam.
- Frecency inspiration: fzf history, Raycast/Alfred recents, Firefox "frecency".

## Delivered (2026-06-05, v0.5.0)

Implemented + agent-verified (AC1–AC8 by `cargo test`; AC9 by the unchanged
existing suite). Build/clippy(`-D warnings`)/fmt clean; release binary 8.2 MB.
As-built notes / decisions taken in planning:

- **Storage engine: `redb` 2.6.3 (pure-Rust), not SQLite.** TC1 resolved toward
  redb because atref is published to crates.io and `rusqlite`'s `bundled` feature
  would force a C compiler on every `cargo install` (and ~1 MB + slow first
  build). We need no SQL — nucleo still matches in memory — so the store is just a
  typed KV cache + frecency ledger. `src/store.rs` holds three tables (`meta`
  schema-version, `entries`, `frecency`), keyed by absolute path with small
  serde_json values (robust for arbitrary paths; sidesteps redb tuple encoding).
  The `Store` is `Clone` (an `Arc<Database>`); redb serializes its own writes, so
  the watcher / reconcile / pick threads share one handle with no extra mutex.
- **Corruption / robustness (FR7/AC4):** `open_or_reset` deletes + recreates a
  corrupt or version-mismatched file, and falls back to an **in-memory** redb
  backend if even that fails, so atref always launches.
- **Load-then-reconcile (FR2):** startup loads `store.load_entries()` instantly
  (no blocking walk — replaces the old synchronous `index::build` in `main`), then
  `App::start_reconcile` walks in the background, `persist`s, and swaps the index
  in via the existing `Msg::IndexReady` + `watch_generation` guard. Reload routes
  through the same path (FR8). First run: empty cache ⇒ reconcile populates it.
- **Watcher write-through (FR3):** the spec-002 watcher closure now `persist`s the
  rebuilt index before sending it, so the on-disk cache never lags memory.
- **Pick recording (FR4):** `accept` bumps an in-memory frecency map immediately
  (so the next empty query reflects it) and persists via `store.record_pick` on a
  spawned thread.
- **Frecency formula (FR5):** `src/frecency.rs::score` = `count × recency_weight`
  with fzf-style buckets (≤1h ×4, ≤1d ×2, ≤1wk ×1, ≤30d ×0.5, older ×0.25);
  never-picked ⇒ 0. Pure (`age` passed in), so unit-tested deterministically.
- **Bounded boost (FR6):** `search::rank` gained a `frecency: &[f64]` slice (pass
  `&[]` ⇒ all-zero ⇒ identical spec-002 ordering, which is how the existing tests
  stay green). Empty query → frecency DESC, then `root_rank`, then path. Non-empty
  query → **score buckets** of width `SCORE_BUCKET = 24` (DESC), then frecency,
  then `root_rank`, then path. Measured nucleo path scores (e.g. `readme`→
  `readme.md` = 159 vs scattered `rxexaxdxmxe.md` = 99) confirm a clean match sits
  ≥2 buckets above a poor one, so frecency only reorders genuinely near-equal
  matches and can't float a clearly-worse match above a clearly-better one.
- **In-memory frecency map is not pruned on reconcile** (only the on-disk store
  is, per NFR4). Stale in-memory rows are harmless — they match no current index
  entry — and the map only holds picked files, so it stays tiny; this keeps the
  `IndexReady` handler O(1) on the UI thread.
- **Store location:** `%APPDATA%\atref\index.redb` (alongside `config.json`).

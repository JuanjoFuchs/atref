---
id: "002"
title: atref result quality — git-aware indexing, folder priority, live file-watching, CamelHumps
status: complete
blocked_by: []
blocks: []
---

# atref Result Quality

## Overview

Spec 001 shipped a working picker, but it indexes with a hand-maintained
`exclude` list, breaks ranking ties arbitrarily, and only refreshes the index
at launch or on manual *Reload*. This spec improves **what** the picker
surfaces, **in what order**, and **how fresh** it is — with no change to the
v0.1 UX:

- **Git-aware indexing** — respect `.gitignore` so build/dependency noise is
  excluded automatically, while still surfacing brand-new non-ignored files.
- **Folder priority** — break ranking ties by the order folders are listed in
  config (earlier = higher priority).
- **Live file-watching** — new / renamed / deleted files appear in (or leave)
  the picker automatically, without *Reload* or restart.
- **CamelHumps verification** — confirm, and tune if needed, initialism /
  CamelCase matching (`fsfg` → `Finite Seasons Family Gift`).

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified — the indexing, ranking, watcher, and matching ACs through
> `cargo test` and runtime checks, plus the regression smoke pass. Build-only
> verification is insufficient. The agent must iterate until verification
> passes.

## Goals

- Cut index noise to "things you'd actually reference" by following Git's own
  ignore rules, with a toggle.
- Make multi-folder ranking predictable: higher-listed folders win ties.
- Keep the in-memory index fresh in real time so new files are immediately
  pickable.
- Confirm the matcher already supports initialism/CamelHumps queries; tune if
  it does not.
- Preserve every spec 001 behavior (chord, picker, insertion, config, tray)
  unchanged.

## Requirements

### Functional Requirements

- **FR1 — Git-aware walking.** When indexing a configured folder inside a Git
  working tree, exclude paths matched by the applicable Git ignore rules
  (`.gitignore` at any level, `.git/info/exclude`, and the user's global
  gitignore). Files that are merely untracked — new, not yet committed — but
  not ignored ARE indexed.
- **FR2 — Git-aware toggle.** A config flag `git_aware` (default `true`)
  enables FR1. When it is `false`, or when a folder is not in a Git working
  tree, that folder falls back to plain recursive enumeration.
- **FR3 — Manual exclude still applies.** The existing `exclude`
  directory-name list is pruned in both modes, as an overlay on top of Git's
  rules.
- **FR4 — Hidden files still skipped.** Dot-prefixed and Windows-hidden files
  remain excluded (spec 001 behavior) in both modes.
- **FR5 — Folder-priority ranking.** Results are ordered primarily by match
  score; ties are broken by the position of the file's root in the `folders`
  list (earlier wins), then by path. With an empty query, files are listed in
  folder-priority order.
- **FR6 — Live file-watching.** While running, atref watches every configured
  folder recursively. Filesystem create / delete / rename / move events update
  the in-memory index so affected files become — or stop being — searchable
  without *Reload* or restart. Watched updates honor FR1–FR4 (a newly-created
  gitignored or hidden file is NOT added).
- **FR7 — Debounce.** Filesystem event bursts are debounced into at most one
  index update per short interval; watching never freezes or blocks the picker.
- **FR8 — Reload extends to the watcher.** Tray *Reload config* continues to
  re-read config, rebuild the index, and re-register the chord (spec 001 FR2),
  and now also restarts the watcher against the new folder set.
- **FR9 — CamelHumps / initialism matching.** Typing the leading letters of a
  name's words matches that name — across CamelCase boundaries
  (`mclfi` → `MyClassFile`) and separator boundaries (space / `_` / `-`,
  e.g. `fsfg` → `Finite Seasons Family Gift`). The target ranks at or near the
  top for such a query.

### Non-Functional Requirements

- **NFR1 — Watcher latency.** A newly-created non-ignored file appears in the
  picker within 2 seconds of creation on a warm filesystem.
- **NFR2 — No UI stall.** Watcher-driven index updates do not block the picker;
  per-keystroke filter latency stays within the spec 001 budget.
- **NFR3 — Indexing throughput.** Git-aware indexing of a ≥10,000-file tree
  completes within spec 001's startup budget — no worse than the `walkdir`
  baseline.

### Technical Constraints

- **TC1 — Libraries.** Use the `ignore` crate (ripgrep's) for git-aware
  walking, replacing `walkdir`; use the `notify` crate (with a debouncer) for
  filesystem watching. Other spec 001 dependencies are unchanged.
- **TC2 — Platform.** Windows 10 (19041+) / 11, x64 — inherited from spec 001.
- **TC3 — Thread-safety.** The index is read by the UI thread and mutated by
  the watcher; access must be synchronized without blocking the UI. The
  mechanism is the implementer's choice.
- **TC4 — Config compatibility.** Existing `config.json` files keep working;
  `git_aware` is optional and defaults to `true` when absent.

## Key Decisions

- **`ignore` over a hand-rolled gitignore parser.** It is the same engine
  ripgrep uses — correct across nested `.gitignore`, `.git/info/exclude`, and
  global gitignore — and it yields untracked-but-not-ignored files, which is the
  desired semantics (new files show up immediately; only *ignored* noise is
  dropped).
- **In-memory index stays; the watcher keeps it fresh.** A persistent on-disk
  index (SQLite/FTS5) is deferred (Roadmap #29); this spec solves freshness
  with live watching rather than persistence.
- **Single global `git_aware` flag, not per-folder.** Smallest config that
  covers the need; per-folder overrides are out of scope.

## Pre-requisites (Human Required)

- [x] Spec 001 complete (it is).
- [x] A configured folder that is a Git repo with a `.gitignore` (e.g.
      `D:\jfuchs\dev\atref`) available for verifying FR1.

## Implementation Tasks

- [x] Replace the `walkdir` indexer with the `ignore` crate; apply Git ignore
      rules when `git_aware` is on and the folder is in a Git tree, else plain
      walk. Keep the `exclude` overlay and hidden-file filter (FR1–FR4).
- [x] Add `git_aware` to the config schema, default `true` when absent
      (FR2, TC4).
- [x] Add folder-priority tiebreaking to result ranking (FR5).
- [x] Add a `notify`-based recursive watcher over all configured folders, with
      debounce, applying the same filters to events and updating the index live
      (FR6, FR7, NFR1, NFR2).
- [x] Restart the watcher on *Reload config* against the new folder set (FR8).
- [x] Synchronize index access between watcher and UI thread without blocking
      (TC3).
- [x] Add tests: gitignored-excluded + untracked-shown (FR1), toggle off (FR2),
      folder-priority ordering (FR5), initialism ranking (FR9), and watcher
      add/remove over a temp directory (FR6 / NFR1).

## Acceptance Criteria

### Git-aware indexing

- [x] **AC1**: With `git_aware: true` and a folder that is a Git repo, files
      matched by `.gitignore` are absent from the index while a newly-created
      non-ignored file IS present.
- [x] **AC2**: With `git_aware: false`, gitignored files reappear (fallback to
      plain enumeration + `exclude`).
- [x] **AC3**: The `exclude` list and hidden-file skipping still apply when
      `git_aware: true`.
- [x] **AC4**: A `config.json` with no `git_aware` key loads and behaves as
      `git_aware: true`.

### Folder priority

- [x] **AC5**: Given two folders each containing an equally-scoring match, the
      file under the earlier-listed folder ranks first; reversing the folder
      order reverses the ranking.

### Live file-watching

- [x] **AC6**: Creating a new non-ignored file in a watched folder makes it
      appear in the picker within 2 seconds, with no *Reload* (NFR1).
- [x] **AC7**: Deleting or renaming a file removes the stale entry within 2
      seconds.
- [x] **AC8**: Creating a gitignored or hidden file does NOT add it (the
      watcher honors FR1–FR4).
- [x] **AC9**: A burst of file changes neither panics nor visibly lags the
      picker (NFR2, FR7).

### CamelHumps

- [x] **AC10**: Query `mclfi` ranks `MyClassFile.*` first among distractors,
      and query `fsfg` ranks `Finite Seasons Family Gift.md` first (FR9).

### Regression

- [x] **AC11**: All spec 001 behavior still works — chord, picker, `@"path"`
      insertion, tray menu, JSON config, and *Reload* (which now also restarts
      the watcher, FR8).

### Validation methods

Per `ai-docs/testable-architecture.md` — this spec is fully agent-verifiable
except a short regression smoke.

| AC | Method |
|---|---|
| AC1–AC4 (git-aware indexing) | `integration` — temp Git repo with a `.gitignore` |
| AC5 (folder priority) | `unit` |
| AC6–AC8 (file-watching) | `integration` — temp dir + `notify`, poll-to-converge |
| AC9 (burst) | `integration` — stress; assert no panic + convergence |
| AC10 (CamelHumps) | `unit` |
| AC11 (regression) | `unit` (picker state model) + a short `manual` smoke |

## Testing Approach

### Validation Steps

1. **Build / lint / format:** `cargo build`, `cargo clippy --all-targets --
   -D warnings`, `cargo fmt --check` — all clean.
2. **Unit / integration tests:** `cargo test` covering git-aware filtering
   (AC1–AC4), folder priority (AC5), initialism (AC10), and watcher add/remove
   over a temp directory (AC6–AC8). These are headless and agent-verifiable.
3. **Throughput:** index a ≥10,000-file tree and confirm startup stays within
   the spec 001 budget (NFR3).
4. **Burst:** programmatically create many files at once; assert no panic and
   that the index converges; observe no UI lag (AC9).
5. **Regression smoke (light human pass):** launch, chord, filter, pick,
   insert, *Reload* (AC11).

### Test Cases

| Input | Expected |
|-------|----------|
| Temp Git repo: `node_modules/` gitignored + new `note.md` | `note.md` indexed; `node_modules` absent |
| Same repo, `git_aware: false` | `node_modules` reappears |
| Two folders, equal-scoring file in each | earlier folder's file ranks first |
| Create a non-ignored file while running | appears within 2 s |
| Delete a file while running | gone within 2 s |
| Create a gitignored file while running | not added |
| Query `fsfg` over `Finite Seasons Family Gift.md` | that file ranks first |

### Human-in-the-Loop Testing Protocol

The indexing, ranking, watcher, and matcher layers are headless-testable, so
the agent verifies AC1–AC10 itself. Hand off to JJ only for the regression
smoke (AC11) and a quick "create a file and watch it appear in the live picker"
confirmation; iterate on any failure.

## Usage Examples

```json
// %APPDATA%\atref\config.json
{
  "folders": ["D:\\jfuchs\\dev\\second-brain", "D:\\jfuchs\\dev\\atref"],
  "chord": "Control+Space",
  "exclude": [".obsidian"],
  "git_aware": true
}
```

With `git_aware: true` you can drop `node_modules` / `target` / `.git` from
`exclude` — Git already ignores them. Keep `exclude` for noise Git does *not*
ignore (e.g. `.obsidian`).

## Out of Scope

- Persistent / on-disk index (SQLite FTS5) — Roadmap #29, a later spec. The
  index stays in-memory, now kept fresh by the watcher.
- Match-position highlighting (#11) and frecency / recents (#24).
- Any picker UI / visual change — that is spec 003.
- Format cycling (#14), settings GUI (#16), `@`-keystroke trigger (#23).
- Per-folder `git_aware` overrides (single global flag only).
- Network-drive / UNC watch-reliability tuning.

## References

- Project node: `D:\jfuchs\dev\second-brain\📦 atref.md` — Roadmap #8, #9,
  #10, #19.
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`.
- Builds on: `001-windows-mvp.md` (complete).
- Crates: [`ignore`](https://docs.rs/ignore/) (ripgrep's walker),
  [`notify`](https://docs.rs/notify/).

---
id: "006"
title: atref config hot-reload — apply config.json edits without a manual Reload
status: pending
blocked_by: []
blocks: ["007"]
---

# atref Config Hot-Reload

## Overview

atref reads `config.json` only at launch and when the user clicks tray →
**Reload config**. This spec makes the resident app **watch its own
`config.json`** and apply edits automatically, running the exact same reload
path the tray menu already triggers (re-read config → re-register chord →
reconcile the index → write through to the store).

This is a standalone win for hand-edits (save the file, it just takes effect),
and it is the enabler for the agent-facing CLI (spec 007): the CLI mutates only
`config.json`, and this watch turns that edit into an index refresh — so no
direct store access and no IPC are needed.

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through `cargo test` (integration against a temp config dir via
> the `ATREF_DIR` seam, exercising the watcher/reload path) plus one `live-gui`
> end-to-end. The error-dialog surface is the only `manual` item. Build-only
> verification is insufficient. The agent must iterate until verification passes.

## Goals

- Edits to `config.json` (by a human editor or another process) take effect in
  the running app without a manual tray Reload.
- Reuse the existing reload → reconcile → store-write-through path; this spec
  adds an automatic trigger, not new reload behavior.
- Be robust to real editor save patterns (in-place writes and atomic
  replace-on-save) and to malformed saves.

## Requirements

### Functional Requirements

- **FR1**: While running, the app detects changes to its `config.json` and
  applies them automatically — equivalent to the user clicking tray → Reload.
- **FR2**: Detection survives both in-place writes **and** atomic
  replace-on-save (write-temp-then-rename), where the file is briefly replaced
  or recreated. (Watching by directory + filename, not by a held file handle.)
- **FR3**: A burst of writes — or a rename followed by a write — coalesces into
  **at most one** reload, fired after the file settles (debounced).
- **FR4**: A malformed save (invalid JSON or invalid field values) does **not**
  crash the app and does **not** discard the running configuration: the
  last-good config stays active, the error is surfaced the same way a manual
  Reload surfaces it (native error dialog), and watching continues so a
  subsequent valid save applies.
- **FR5**: A `folders`/`exclude`/`git_aware` change reconciles the index (adds
  new files, drops removed ones) and writes through to the persistent store —
  identical to a manual Reload.
- **FR6**: A `chord` change re-registers the global hotkey — identical to a
  manual Reload.
- **FR7**: The manual tray **Reload config** item remains available as a
  fallback (unchanged).

### Non-Functional Requirements

- **NFR1**: The watch is event-driven with no measurable idle CPU (consistent
  with the spec-002 content watcher).
- **NFR2**: After the file settles, the reload is unobtrusive (sub-second to
  begin) and never blocks the UI thread on the folder walk (the reconcile runs
  in the background, as today).

### Technical Constraints

- **TC1**: Reuse the existing `notify`-based watch/debounce machinery already in
  the codebase (the spec-002 file watcher) rather than introducing a second
  watch stack.
- **TC2**: The watched file is the active config at `%APPDATA%\atref\config.json`,
  or `$ATREF_DIR/config.json` when `ATREF_DIR` is set (the test-only seam).

## Implementation Tasks

- [ ] Watch the config file's containing directory and react to changes that
      affect `config.json` (covering rename/replace, per FR2).
- [ ] On a settled change, run the existing reload path (re-read config →
      re-register chord → background reconcile → store write-through).
- [ ] On a malformed config, keep the last-good config active, surface the
      error like a manual Reload, and keep watching (FR4).
- [ ] Confirm the manual tray Reload still works (FR7).

## Acceptance Criteria

### Core behavior
- [ ] **AC1** (`integration`): With the app's watch active over a temp
      `ATREF_DIR`, an in-place edit to `folders` reconciles the index to the new
      set within the debounce window — a file under an added folder becomes
      indexed and a file under a removed folder drops. (FR1, FR5)
- [ ] **AC2** (`integration`): An atomic replace-on-save of `config.json`
      (write temp + rename over it) triggers the same reconcile, proving
      directory+filename watching rather than a held handle. (FR2)
- [ ] **AC3** (`integration`): A rapid burst of writes produces exactly one
      reconcile after settling, not one per write. (FR3)
- [ ] **AC4** (`integration`): Saving malformed JSON does not panic and leaves
      the previously-indexed folder set unchanged; a subsequent valid save then
      applies. (FR4)
- [ ] **AC5** (`integration`): Changing `chord` in `config.json` results in the
      newly-configured chord being the registered hotkey. (FR6)

### End-to-end
- [ ] **AC6** (`live-gui`): With the app running under the live-GUI harness, an
      external process appends a folder to `config.json`; a subsequent picker
      query then finds a file from that folder, with no manual Reload. (FR1, FR5)

### Surface (irreducible)
- [ ] **AC7** (`manual`): The native error dialog appears on a malformed save.
      Manual because it is an OS dialog; the no-crash / last-good guarantee is
      covered headlessly by AC4.

## Testing Approach

See `ai-docs/testable-architecture.md` (seams + labels) and
`ai-docs/agentic-gui-testing.md` (the `live-gui` lane).

### Validation Steps
1. Build clean: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo build`.
2. `cargo test` — the config-watch integration cases (AC1–AC5) over a temp
   `ATREF_DIR`, mirroring `tests/watcher.rs`.
3. `live-gui` — AC6 end-to-end against the running `.exe`.
4. Manual smoke — AC7 (malformed-save dialog).

### Test Cases
| Change to config.json | Expected result |
|---|---|
| In-place edit adds a folder | Index gains that folder's files (one reconcile) |
| In-place edit removes a folder | Index drops that folder's files |
| Atomic replace-on-save | Same reconcile fires (FR2) |
| 5 rapid writes | Exactly one reconcile after settling |
| Invalid JSON saved | No crash; prior folder set retained; error surfaced |
| `chord` changed | New chord is the active hotkey |

## Out of Scope

- Removing the manual tray **Reload config** item (kept as a fallback).
- Watching anything other than `config.json` (the store file is not watched).
- Config schema versioning / migration.
- Re-rendering an already-open picker mid-edit (a reload affects the index and
  chord; an open picker simply uses the new state on its next query).
- Any CLI surface — that is spec 007, which depends on this one.

## References
- Project node + roadmap: `📦 atref.md` (capability #34; the agent-CLI design
  note dated 2026-06-08) in JJ's vault.
- Enables: spec 007 (agent-facing CLI).
- Prior watcher: spec 002 (`notify` debouncer) and `tests/watcher.rs`.
- Testing seams: `ai-docs/testable-architecture.md`, `ai-docs/agentic-gui-testing.md`.

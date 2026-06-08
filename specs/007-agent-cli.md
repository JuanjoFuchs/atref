---
id: "007"
title: atref agent-facing CLI — add folders and self-describe from the same binary
status: pending
blocked_by: ["006"]
blocks: []
---

# atref Agent-Facing CLI

## Overview

atref launches as a tray app when run with no arguments. This spec adds a small
**agent-drivable CLI** on the *same* binary so an agent (or a human) can manage
the indexed folders and discover the command surface, without ever touching the
running app's store. The motivating workflow: tell any agent *"add your current
working directory to @ref and refresh the index"* — it runs `atref describe` to
learn the surface, then `atref add .`, and the index refreshes on its own.

The CLI mutates **only `config.json`**. The resident app turns that edit into an
index refresh via config hot-reload (spec 006); when no app is running, the edit
applies on the next launch. So there is no direct store access, no
single-writer-lock contention, and no IPC — `config.json` is the single
coordination point.

```
  agent / human                         resident atref (if running)
  ────────────                          ───────────────────────────
  atref add <dir>  ──writes──▶  config.json  ──watch (spec 006)──▶  reconcile + store
  atref describe   ──prints surface──▶  (agent learns commands)
  atref            ──no args──▶  launches the tray app (unchanged)
```

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through `cargo test` (integration against a temp config via the
> `ATREF_DIR` seam) plus one `live-gui` end-to-end proving an external
> `atref add` is picked up by the running app. Build-only verification is
> insufficient. The agent must iterate until verification passes.

## Goals

- One command lets an agent add a folder to atref's index, after discovering the
  surface via `describe` — no vault/source knowledge required.
- The CLI writes only `config.json`; the running app reflects changes via spec
  006, and a not-running app picks them up on next launch.
- No new daemon or IPC; reuse `config.json` as the coordination point.

## Requirements

### Functional Requirements

- **FR1**: `atref describe` prints a self-documenting description of the CLI
  surface — every subcommand, its arguments and defaults, what it does, and the
  config-file location — following the vault's `describe` convention
  (`now.py` / agent-mail). Exit code 0.
- **FR2**: `atref add [PATH]` adds `PATH` to the `folders` array in
  `config.json`, normalized to an absolute path. `PATH` defaults to the current
  working directory. On success it reports what was added; exit code 0.
- **FR3**: `atref add` is idempotent — adding a folder already present (after
  normalization) makes no duplicate entry and reports it as already indexed;
  exit code 0.
- **FR4**: If `config.json` does not exist, `atref add` first creates it with
  the same first-run default the tray app uses, then adds the folder.
- **FR5**: `atref` with no arguments launches the tray app exactly as today (no
  behavior change).
- **FR6**: The CLI never opens the persistent store; it only reads and writes
  `config.json`. It therefore succeeds whether or not a resident app holds the
  store.
- **FR7**: When a resident app is running, an `atref add` is applied
  automatically via config hot-reload (spec 006); the agent runs no separate
  refresh step.
- **FR8**: An unknown subcommand or invalid arguments produce a helpful message
  on stderr and a non-zero exit code; `--help`/`-h` and `--version`/`-V` work.

### Non-Functional Requirements

- **NFR1**: CLI subcommands return promptly — they edit `config.json` only and
  never walk folders or open the store.
- **NFR2**: `describe` output is stable and structured enough for an agent to
  parse the available commands and arguments from it alone.

### Technical Constraints

- **TC1**: The CLI is the **same `atref` binary** dispatched on its arguments —
  not a separate executable. No arguments ⇒ tray app (FR5). (Stated because the
  implementing agent cannot derive the one-binary-vs-two decision from context.)
- **TC2**: Config path is `%APPDATA%\atref\config.json`, or
  `$ATREF_DIR/config.json` when `ATREF_DIR` is set (the test seam).
- **TC3**: Automatic refresh (FR7) depends on spec 006 (config hot-reload);
  this spec is `blocked_by: ["006"]`.

## CLI Surface (contract)

```
atref                 # launch the tray app (default; unchanged)
atref describe        # print the command surface (subcommands, args, config path)
atref add [PATH]      # add PATH (default: current dir) to the indexed folders
atref --help | -h     # usage
atref --version | -V  # version
```

`describe` output is text (matching the vault's `describe` tools), structured so
each subcommand's name, syntax, and purpose are individually parseable, and it
states the config path. Representative shape (exact wording derived from the
existing convention):

```
atref — global file-reference picker. CLI for agents.
config: C:\Users\<you>\AppData\Roaming\atref\config.json

COMMANDS
  add [PATH]   Add PATH (default: current directory) to the indexed folders.
               Writes config.json; a running atref picks it up automatically.
  describe     Print this surface.

Run `atref` with no arguments to launch the tray app.
```

## Implementation Tasks

- [ ] Dispatch on arguments: no args ⇒ tray app (today's path); a recognized
      subcommand ⇒ run it and exit; unknown ⇒ error + non-zero exit.
- [ ] `add [PATH]`: resolve to an absolute path, load-or-create `config.json`
      with the first-run default, append to `folders` if not already present,
      write back, report the outcome.
- [ ] `describe`: print the self-documenting surface and the config path.
- [ ] Ensure neither subcommand opens the store (FR6) and neither walks folders.

## Acceptance Criteria

### add
- [ ] **AC1** (`integration`): On a temp `ATREF_DIR` with no config, `atref add <dir>`
      creates `config.json` from the default and `folders` contains `<dir>`
      (absolute). (FR2, FR4)
- [ ] **AC2** (`integration`): Running `atref add <dir>` again does not duplicate
      the entry and reports it as already indexed; exit 0. (FR3)
- [ ] **AC3** (`integration`): `atref add` with no PATH adds the current working
      directory as an absolute path. (FR2 default)
- [ ] **AC4** (`integration`): A relative or non-normalized PATH is stored as a
      normalized absolute path. (FR2)
- [ ] **AC5** (`integration`): `atref add` succeeds while another open handle
      holds the store file, confirming the CLI never contends the store lock.
      (FR6)

### describe & dispatch
- [ ] **AC6** (`integration`): `atref describe` exits 0 and its output names the
      `add` and `describe` subcommands and the config path. (FR1)
- [ ] **AC7** (`integration`): An unknown subcommand exits non-zero with a stderr
      message; `--version` prints the crate version; `--help` prints usage. (FR8)
- [ ] **AC8** (`live-gui`): `atref` no-args still launches the tray app — covered
      by the live-GUI harness, which starts the no-arg `.exe` and drives it. (FR5)

### end-to-end (with spec 006)
- [ ] **AC9** (`live-gui`): With the app running, a separate `atref add <dir>`
      process is picked up automatically (spec 006) and a subsequent picker query
      finds a file from `<dir>` — no manual Reload, no separate refresh. (FR7)

## Testing Approach

See `ai-docs/testable-architecture.md` and `ai-docs/agentic-gui-testing.md`.

### Validation Steps
1. Build clean: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo build`.
2. `cargo test` — `add`/`describe`/dispatch integration cases over a temp
   `ATREF_DIR` (AC1–AC7).
3. `live-gui` — AC8 (no-arg launch) and AC9 (external `add` auto-applied).

### Test Cases
| Command | Config state | Expected |
|---|---|---|
| `atref add D:\proj` | no config | config created (default); `folders` has `D:\proj` |
| `atref add D:\proj` | already has `D:\proj` | no duplicate; "already indexed"; exit 0 |
| `atref add` (in `D:\proj`) | any | adds `D:\proj` (CWD), absolute |
| `atref describe` | any | exit 0; lists `add`, `describe`, config path |
| `atref frobnicate` | any | stderr error; non-zero exit |
| `atref` | any | tray app launches (unchanged) |

## Usage Examples

```
# An agent, told "add this folder to @ref and refresh":
atref describe          # learn the surface
atref add               # add the current working directory
# (a running atref reconciles automatically via spec 006)

atref add D:\jfuchs\dev\some-repo   # add a specific folder
```

## Out of Scope

- `atref remove` and `atref list` — only `add` and `describe` here.
- A separate `atref reindex` / refresh command — unnecessary; refresh is
  automatic via spec 006 (and on next launch when not running).
- Editing any config field other than `folders` (e.g. `chord`, `exclude`,
  `git_aware`) via the CLI.
- Any direct store mutation from the CLI (explicitly forbidden — FR6).
- Daemon or IPC between the CLI and the resident app.
- Non-Windows CLI behavior beyond what the shared codebase already provides
  (Windows-first, like the rest of atref).

## References
- Project node + roadmap: `📦 atref.md` (capability #33; the agent-CLI design
  note dated 2026-06-08 — config.json as the single coordination point) in JJ's vault.
- Depends on: spec 006 (config hot-reload) for FR7's automatic refresh.
- `describe` convention: the vault's `now.py` / `vault-graph.py` / agent-mail tools.
- Testing seams: `ai-docs/testable-architecture.md`, `ai-docs/agentic-gui-testing.md`.

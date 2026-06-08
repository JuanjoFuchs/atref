---
id: "007"
title: atref agent-facing CLI — fully configure atref from the same binary
status: complete
blocked_by: ["006"]
blocks: []
---

# atref Agent-Facing CLI

## Overview

atref launches as a tray app when run with no arguments. This spec adds an
**agent-drivable CLI** on the *same* binary so an agent (or a human) can **fully
configure** atref — `folders`, `exclude`, `chord`, `git_aware` — through a
uniform, **validating** `config` namespace, and discover the whole surface via a
JSON `describe` that doubles as the schema.

The CLI validates every change with atref's own logic and writes **only**
`config.json` — never the persistent store. The resident app turns that edit into
an applied change via config hot-reload (spec 006); when no app is running, the
edit applies on the next launch. So there is no direct store access, no
single-writer-lock contention, and no IPC — `config.json` is the single
coordination point.

```
  agent / human                              resident atref (if running)
  ────────────                               ───────────────────────────
  atref config set/add/remove ─validate→write→ config.json ─watch (006)→ reconcile + re-register
  atref config get / describe ─prints JSON──▶  (agent reads state + schema)
  atref                       ─no args──────▶  launches the tray app (unchanged)
```

Why a CLI rather than letting agents edit `config.json` directly: atref surfaces
a bad config only as a native dialog and silently keeps the last-good config on
reload, so a direct edit that is subtly wrong (e.g. an unparseable `chord`) gives
the agent **no signal**. The CLI validates up front and returns an exit code plus
machine-readable JSON, matching the vault's agent-CLI doctrine (machine-readable
output, strict validation, a `describe`/schema command, safety rails on mutations).

> **Completion rule:** This spec is not complete until all acceptance criteria
> are verified through `cargo test` (integration over a temp config via the
> `ATREF_DIR` seam) plus the `live-gui` end-to-end cases. Build-only verification
> is insufficient. The agent must iterate until verification passes.

## Goals

- An agent can read, add, remove, and change **every** atref setting with one
  command each, after discovering the surface and schema via `describe`.
- Every mutation is validated with atref's own logic and reports success/failure
  in the agent's own channel (exit code + JSON) — never a silent no-op.
- The CLI writes only `config.json`; a running app reflects changes via spec 006,
  and a not-running app picks them up on next launch. No daemon, no IPC.

## Requirements

### Functional Requirements

- **FR1**: `atref describe` prints, as JSON, a self-documenting schema: tool name,
  usage, every command (with its arguments), and **every config field** keyed by
  name with its `kind` (`list` | `scalar`), value type, default, and validation
  rule — plus the resolved config-file path. Exit 0. `--human` prints a readable
  variant. (This is the schema; there is no separate schema file.)
- **FR2**: `atref config get [KEY]` prints the current configuration as JSON — the
  whole config, or just `KEY` when given. Unknown `KEY` → error + non-zero exit.
- **FR3**: `atref config set <KEY> <VALUE>` sets a **scalar** field (`chord`,
  `git_aware`). The result is validated before any write; on success the new value
  is persisted and reported; on invalid input nothing is written and a non-zero
  exit + message results.
- **FR4**: `atref config add <KEY> <VALUE>` adds `VALUE` to a **list** field
  (`folders`, `exclude`). Idempotent — adding a value already present makes no
  duplicate and reports it as already present. `folders` values are normalized to
  absolute paths.
- **FR5**: `atref config remove <KEY> <VALUE>` removes `VALUE` from a **list**
  field. Removing an absent value is a no-op success (idempotent).
- **FR6**: Every mutation validates the *resulting* configuration with atref's own
  validation (the same rules the app applies on load), and the `chord` value with
  the same parser the app uses to register it, **before** writing. An invalid
  result is never persisted. Writes are atomic (a partial/half-written
  `config.json` is never observable to the spec-006 watcher).
- **FR7**: Safety rails on mutations: `remove folders` that would empty `folders`
  is refused (it must stay non-empty) → message + non-zero exit, no write; an
  unparseable `chord` is rejected with the parser's exact error; a non-boolean
  `git_aware` is rejected.
- **FR8**: `atref` with no arguments launches the tray app exactly as today (no
  behavior change).
- **FR9**: The CLI never opens the persistent store; it only reads/writes
  `config.json`, so it succeeds whether or not a resident app holds the store.
- **FR10**: When a resident app is running, mutations are applied automatically via
  config hot-reload (spec 006); the agent runs no separate refresh. When not
  running, they apply on next launch.
- **FR11**: `atref add [PATH]` is a shortcut for `config add folders <PATH>`, where
  `PATH` defaults to the current working directory.
- **FR12**: If `config.json` does not exist, a mutating command first creates it
  from the same first-run default the tray app uses, then applies the change.
- **FR13**: Output is machine-readable JSON on stdout by default (a result object
  carrying the action, key, resulting value, whether anything changed, and the
  config path; or an error object with a message). Errors/usage go to stderr with a
  non-zero exit. Unknown subcommand/key or bad args → helpful message + non-zero
  exit; `--help`/`-h` and `--version`/`-V` work.

### Non-Functional Requirements

- **NFR1**: CLI commands return promptly — they touch only `config.json` and never
  walk folders or open the store.
- **NFR2**: `describe` and result output are valid, stable JSON an agent can parse
  to learn the surface, the field schema, and the outcome of a mutation.

### Technical Constraints

- **TC1**: The CLI is the **same `atref` binary** dispatched on its arguments — not
  a separate executable. No arguments ⇒ tray app (FR8). (Stated because the
  implementing agent cannot derive the one-binary-vs-two decision from context.)
- **TC2**: Config path is `%APPDATA%\atref\config.json`, or `$ATREF_DIR/config.json`
  when `ATREF_DIR` is set (the test seam).
- **TC3**: Automatic refresh (FR10) depends on spec 006 (config hot-reload); this
  spec is `blocked_by: ["006"]`.
- **TC4**: `describe` *is* the schema — do not introduce a separate JSON-Schema
  file or a schema-generation dependency (matches the vault's agent-CLI convention,
  which uses describe-emitted schema dicts, not `.schema.json`).

## CLI Surface (contract)

```
atref                              # launch the tray app (default; unchanged)
atref describe [--human]           # JSON schema: commands + config fields + config path
atref config [get [KEY]]           # print the whole config, or one KEY, as JSON
atref config set <KEY> <VALUE>     # scalar fields: chord, git_aware
atref config add <KEY> <VALUE>     # list fields: folders, exclude   (idempotent)
atref config remove <KEY> <VALUE>  # list fields: folders, exclude   (idempotent)
atref add [PATH]                   # shortcut: config add folders <PATH | current dir>
atref --help | -h  ·  atref --version | -V
```

Config fields (the schema `describe` reports):

| Key | Kind | Value type | Default | Validation |
|-----|------|-----------|---------|-----------|
| `folders` | list | absolute path | `[home]` | must stay non-empty; values normalized to absolute |
| `exclude` | list | string | `[.git, node_modules, target]` | — |
| `chord` | scalar | string | `Control+Space` | must parse as a global-hotkey chord |
| `git_aware` | scalar | bool | `true` | must be a boolean |

Representative `describe` output (exact wording derived from the existing
`now.py` / `vault-graph.py` convention — JSON to stdout):

```json
{
  "name": "atref",
  "description": "Global file-reference picker; CLI to configure it.",
  "usage": "atref <command> [args] [--human]",
  "config_path": "C:\\Users\\<you>\\AppData\\Roaming\\atref\\config.json",
  "commands": {
    "describe": "Print this schema as JSON.",
    "config get [KEY]": "Print the whole config, or one KEY, as JSON.",
    "config set <KEY> <VALUE>": "Set a scalar field (chord, git_aware).",
    "config add <KEY> <VALUE>": "Add to a list field (folders, exclude).",
    "config remove <KEY> <VALUE>": "Remove from a list field.",
    "add [PATH]": "Shortcut: add PATH (default current dir) to folders.",
    "(no args)": "Launch the tray app."
  },
  "fields": {
    "folders":   { "kind": "list",   "type": "abs path", "default": "[home]",                    "validation": "non-empty; absolute" },
    "exclude":   { "kind": "list",   "type": "string",   "default": "[.git, node_modules, target]" },
    "chord":     { "kind": "scalar", "type": "string",   "default": "Control+Space",             "validation": "parses as a chord" },
    "git_aware": { "kind": "scalar", "type": "bool",     "default": true }
  }
}
```

Representative mutation result (stdout, exit 0) and error (stderr, non-zero):

```json
{ "ok": true,  "action": "add", "key": "folders", "value": "D:\\proj", "changed": true, "config_path": "..." }
{ "ok": false, "action": "set", "key": "chord", "error": "invalid chord '@@@': <parser message>" }
```

## Implementation Tasks

- [x] Dispatch on arguments: no args ⇒ tray app (today's path); a recognized
      subcommand ⇒ run it and exit; unknown ⇒ error + non-zero.
- [x] Implement `describe` (JSON schema dict + config path; `--human` variant).
- [x] Implement `config get [KEY]`, `set`, `add`, `remove` over `config.json`,
      reusing the app's load/validate logic and the chord parser; load-or-create
      from the first-run default; write atomically only when the result validates.
- [x] Enforce the safety rails (non-empty `folders`, chord parse, bool `git_aware`)
      and idempotency for list add/remove; keep the `atref add [PATH]` shortcut.
- [x] Ensure no command opens the store or walks folders.

## Acceptance Criteria

### get / describe / dispatch
- [x] **AC1** (`integration`): `config get` prints valid JSON equal to the parsed
      `config.json`; `config get folders` prints just that field; unknown KEY →
      non-zero + message. (FR2)
- [x] **AC2** (`integration`): `describe` exits 0, output is valid JSON, and it
      names every command and every field with its `kind`/type/default plus the
      config path. (FR1)
- [x] **AC3** (`integration`): unknown subcommand → non-zero + stderr message;
      `--version` prints the crate version; `--help` prints usage. (FR13)
- [x] **AC4** (`integration`): a mutating command with no existing `config.json`
      creates it from the first-run default, then applies the change. (FR12)

### set (scalars)
- [x] **AC5** (`integration`): `config set git_aware false` persists `false`;
      `config get git_aware` reflects it. A non-bool value → non-zero, message,
      file unchanged. (FR3, FR7)
- [x] **AC6** (`integration`): `config set chord "Control+Shift+P"` (valid)
      persists it; `config set chord "@@@"` (unparseable) → non-zero with the
      parser's error and the file unchanged. (FR3, FR6, FR7)

### add / remove (lists)
- [x] **AC7** (`integration`): `config add folders <dir>` stores the absolute path;
      running it again is idempotent (no duplicate, exit 0); a relative path is
      stored absolute. (FR4)
- [x] **AC8** (`integration`): `config add exclude node_modules` adds the value
      idempotently; `config remove exclude target` removes it, and removing an
      absent value is a no-op success. (FR4, FR5)
- [x] **AC9** (`integration`): `config remove folders <last>` is refused (non-empty
      rail) → non-zero + message, file unchanged; removing a non-last folder
      succeeds. (FR7)
- [x] **AC10** (`integration`): `atref add <dir>` equals `config add folders <dir>`,
      and `atref add` with no PATH adds the current working directory (absolute).
      (FR11)

### invariants
- [x] **AC11** (`integration`): a mutation that would yield an invalid config never
      writes (file byte-identical on failure); a successful write is complete and
      parseable (atomic). (FR6)
- [x] **AC12** (`integration`): a mutation succeeds while another open handle holds
      the store file, confirming the CLI never opens/contends the store. (FR9)

### end-to-end (with spec 006)
- [x] **AC13** (`live-gui`): `atref` no-args launches the tray app — covered by the
      live-GUI harness, which starts the no-arg `.exe` and drives it. (FR8)
- [x] **AC14** (`live-gui`): with the app running, an external `atref config add
      folders <dir>` is applied automatically (spec 006) — a subsequent picker query
      finds a file from `<dir>` with no manual Reload. (FR10)

## Delivered (2026-06-08)

The CLI is the `atref` binary dispatched on argv (no args = tray, unchanged); the
pure logic lives in the lib `cli` module and returns stdout/stderr/exit-code so
`main` is a thin I/O shell that never opens the store. It loads-or-defaults
`config.json`, mutates a parsed `Config`, validates with the app's own rules
(`Config::validate` + `HotKey::from_str` for chords) and writes atomically
(temp + rename). A best-effort `AttachConsole(ATTACH_PARENT_PROCESS)` shim lets a
release build print to a real terminal without clobbering piped output.
`describe` emits a JSON schema dict (commands + field kinds/types/defaults + the
config path), matching the vault convention — no `.schema.json`. Verified by
`src/cli.rs` unit tests (9) + `tests/cli.rs` subprocess tests (5, incl.
succeeds-while-store-held) for AC1–AC12, and `tests/e2e.rs` for AC13 (no-arg
launches the tray) + AC14 (an external `atref config add` auto-applies via
spec 006).

## Testing Approach

See `ai-docs/testable-architecture.md` and `ai-docs/agentic-gui-testing.md`.

### Validation Steps
1. Build clean: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo build`.
2. `cargo test` — `get`/`set`/`add`/`remove`/`describe`/dispatch integration cases
   over a temp `ATREF_DIR` (AC1–AC12).
3. `live-gui` — AC13 (no-arg launch) and AC14 (external mutations auto-applied).

### Test Cases
| Command | Config state | Expected |
|---|---|---|
| `config get folders` | valid config | that field, as JSON, exit 0 |
| `config set git_aware false` | valid | `git_aware=false` persisted |
| `config set git_aware nope` | valid | non-zero; file unchanged |
| `config set chord "@@@"` | valid | non-zero (parser error); file unchanged |
| `config add folders .\sub` | valid | absolute path added; idempotent on repeat |
| `config remove folders <last>` | one folder | refused; non-zero; file unchanged |
| `config remove exclude ghost` | not present | no-op; exit 0 |
| `describe` | any | valid JSON; all commands + fields + config path |
| `atref frobnicate` | any | stderr error; non-zero |
| `atref` | any | tray app launches (unchanged) |

## Usage Examples

```
# An agent, told "add this folder to @ref and refresh":
atref describe                       # learn the surface + schema (config path, fields)
atref add                            # add the current working directory to folders
# (a running atref reconciles automatically via spec 006)

# Fully configure:
atref config add folders D:\jfuchs\dev\some-repo
atref config add exclude dist
atref config set chord "Control+Alt+Space"
atref config set git_aware false
atref config get                     # confirm the resulting config (JSON)
```

## Out of Scope

- A separate JSON-Schema file (`config.schema.json`) or a schema-generation
  dependency (e.g. `schemars`) — `describe` is the schema (TC4).
- Editing config fields that do not exist today; new fields become configurable
  for free as they are added to the config (no new verbs needed).
- `atref list` as a distinct command — `config get` covers reading.
- Any direct store mutation from the CLI (explicitly forbidden — FR9).
- A daemon or IPC between the CLI and the resident app.
- Non-Windows CLI behavior beyond what the shared codebase already provides
  (Windows-first, like the rest of atref).

## References
- Project node + roadmap: `📦 atref.md` (capability #33; the agent-CLI design
  note dated 2026-06-08 — config.json as the single coordination point; the CLI
  decision and describe-as-schema choice) in JJ's vault.
- Depends on: spec 006 (config hot-reload) for FR10's automatic refresh.
- `describe`/schema convention: the vault's `now.py` / `vault-graph.py` /
  agent-mail tools (JSON schema dict from `describe`); `CLI Tools.md` doctrine.
- Testing seams: `ai-docs/testable-architecture.md`, `ai-docs/agentic-gui-testing.md`.

---
id: "001"
title: atref Windows MVP — chord-triggered file picker with absolute-path insertion
status: pending
blocked_by: []
blocks: []
---

# atref Windows MVP

## Overview

This spec defines the behavioral contract for the first runnable version
of `atref` on Windows. The goal is the smallest end-to-end slice that
proves the concept: a user runs `atref.exe run` in a terminal, presses a
configured chord (default `Ctrl+Space`) while focused in any application,
sees a borderless fuzzy picker appear near the mouse cursor, types to
filter against the contents of one configured folder, and on `Enter`
gets the file's absolute path inserted at the caret of whichever
application was focused.

This spec is intentionally narrow. It does not cover: file-watcher
indexing, persistent SQLite/FTS5 index, the `@` keystroke trigger, macOS
or Linux, multiple folder profiles, format cycling at selection time
(wikilink / relative path / custom templates), daemon and auto-start,
caret anchoring via UI Automation, `winget` / `brew` / `npx`
distribution, or any UI polish beyond a functional picker. Each of those
is a follow-up spec.

> **Completion rule:** This spec is not complete until every Acceptance
> Criterion is verified through the Testing Approach below, including
> the human-in-the-loop manual tests. Build-only verification is
> insufficient. The agent must iterate until verification passes.

## Goals

- Prove that the four hard primitives — chord registration, picker
  window, fuzzy filtering, caret insertion — work together on Windows.
- Establish a Rust workspace and crate dependency baseline future specs
  will build on.
- Produce a single static `atref.exe` that can be launched from any
  PowerShell or Windows Terminal session and demonstrates the workflow
  end-to-end without an installer.
- Pick UI, hotkey, matcher, and injection libraries deliberately so
  later specs can extend (multi-OS, daemonize, distribute) without
  rewriting the core.

## Requirements

### Functional Requirements

- **FR1**: Provide a single binary `atref.exe` with subcommands:
  - `atref run` — start the foreground process: load config, build the
    in-memory index, register the chord, show the picker on chord.
  - `atref describe` — emit a single JSON document describing the tool's
    purpose, commands, config-file location and schema, and current
    capabilities. Runs without network access and without an existing
    config file (returns defaults inside the schema).
  - `atref --version` — print the version from `Cargo.toml`.
  - `atref --help` — print top-level help listing all subcommands and
    global flags.
- **FR2**: Read configuration from `%APPDATA%\atref\config.toml`. If the
  file does not exist, `atref run` writes a default config there on
  first launch (with the user's home directory as the indexed root and
  `Ctrl+Space` as the chord) and proceeds with those defaults.
- **FR3**: On `atref run` startup, recursively enumerate every regular
  file under the configured root folder into an in-memory `Vec<PathBuf>`
  index. Symlinks are not followed. Hidden files (starting with `.` on
  any OS, or with the Windows hidden attribute) are skipped. Directories
  matching the configured `exclude` globs (defaults: `.git`,
  `node_modules`, `target`) are pruned during traversal.
- **FR4**: Register a single global keyboard chord via the
  `global-hotkey` crate. Default chord is `Ctrl+Space`. The chord is
  configurable in the TOML file using
  [`HotKey::from_str`](https://docs.rs/global-hotkey/) syntax (e.g.
  `"Control+Space"`, `"Alt+Shift+P"`).
- **FR5**: When the chord fires, show a borderless picker window
  positioned with its top-left corner offset 12 pixels right and 24
  pixels below the current mouse-cursor position. The window must
  appear in front of the previously-focused application without
  stealing focus permanently — the focused application is recorded so
  insertion can target it on dismiss.
- **FR6**: The picker contains, top-to-bottom: a single-line text input
  with the caret in it, and a list of up to 10 matching files. Each
  list row shows the file's basename in normal weight and the parent
  directory (relative to the indexed root) in a dimmer color. The
  currently-selected row is highlighted.
- **FR7**: As the user types in the input, the result list updates per
  keystroke. Matching is performed by `nucleo-matcher` against each
  indexed file's path-from-root, using `nucleo-matcher`'s
  `Config::DEFAULT.match_paths()` settings (basename-weighted, smart
  case, CamelCase aware).
- **FR8**: Keyboard model inside the picker:
  - `↓` / `↑` move the selection one row.
  - `Enter` accepts the selected row.
  - `Esc` dismisses the picker with no insertion.
  - Any other character or `Backspace` edits the filter input.
- **FR9**: On `Enter`, the picker hides immediately, then atref inserts
  the absolute path of the selected file at the caret of the
  previously-focused application by: (a) reading and saving the
  current Windows clipboard contents (text only), (b) writing the
  absolute path to the clipboard, (c) restoring focus to the
  previously-focused window, (d) synthesizing a `Ctrl+V` keystroke,
  (e) waiting 150 ms, (f) restoring the original clipboard text.
- **FR10**: On `Esc`, no insertion occurs, the clipboard is not
  modified, and focus returns to the previously-focused window.
- **FR11**: `atref run` runs in the foreground. `Ctrl+C` in the
  controlling terminal terminates the process cleanly: hotkey is
  unregistered, the picker window is destroyed, and the process exits
  with code 0.

### Non-Functional Requirements

- **NFR1**: Per-keystroke filter latency from key event to repaint is
  under 16 ms (one frame at 60 Hz) on an index of ≤ 10,000 files on a
  modern desktop.
- **NFR2**: From chord press to picker first paint is under 100 ms on
  the same hardware. Achieved by keeping the picker window
  pre-allocated and hidden between activations.
- **NFR3**: Idle memory footprint (`atref run` with picker hidden) is
  under 50 MB resident.
- **NFR4**: `atref.exe` is a single statically-linked binary. No
  external runtime (no `.NET`, no `WebView2`, no Python) is required at
  install time.
- **NFR5**: All structured stdout (`describe`, `--version`, future
  JSON outputs) is valid UTF-8. Errors are printed to stderr.

### Technical Constraints

- **TC1**: Implementation language is Rust, edition 2021, toolchain
  Rust 1.75 or newer (stable).
- **TC2**: Target platform for this spec is Windows 10 (build 19041+)
  and Windows 11, x64 only. Other targets are out of scope.
- **TC3**: Dependency choices (locked in by this spec; alternatives
  require a spec amendment):
  - GUI framework: `eframe` / `egui` ≥ 0.27
  - Global hotkeys: `global-hotkey` ≥ 0.5
  - Fuzzy matcher: `nucleo-matcher` ≥ 0.3
  - Clipboard: `arboard` ≥ 3.4
  - Synthesized keystrokes: `enigo` ≥ 0.3
  - File enumeration: `walkdir` ≥ 2
  - Config / serialization: `toml` ≥ 0.8, `serde` ≥ 1
  - Config-directory resolution: `directories` ≥ 5
  - CLI parsing: `clap` ≥ 4 with `derive` feature
- **TC4**: Single binary, single process for v0.1. No background
  daemon, no Windows service, no auto-start. User must run
  `atref run` from a terminal.
- **TC5**: No installer, no `winget` manifest, no signed executable in
  this spec. `cargo build --release` produces the binary; the user
  runs it from `target\release\atref.exe`.
- **TC6**: The clipboard-paste injection mechanism is brittle: if the
  user runs another clipboard tool that overwrites the clipboard
  inside the 150 ms restore window, the original clipboard contents
  are lost. This is accepted for v0.1 and documented in the README.
  Synthesized keystrokes for arbitrary paths are deferred to a later
  spec (they would require IME-aware character composition).

## Key Decisions

### `egui`/`eframe` over Tauri for v0.1

The strategic vault note targets Tauri for the long-term UI because of
its HTML/CSS styling and built-in auto-updater. For v0.1, `egui` via
`eframe` is chosen instead because: (a) pure Rust, no Node/npm tooling
in the build, (b) no `WebView2` runtime dependency, (c) smaller release
binary (~6 MB vs ~12 MB with Tauri), (d) faster iteration during the
prototype. Tauri remains the candidate for v0.2 if richer styling or
auto-updates are needed. Switching framework is a single-crate concern
and explicitly inside the scope of a future spec.

### Foreground process, no daemon

A daemon, service, or autostart entry is the kind of polish that
matters for a shipped product but obscures whether the core
primitives work. v0.1 requires the user to run `atref run` in a
terminal. The process running in the foreground is also easier to
debug (logs go to stderr, `Ctrl+C` exits).

### In-memory index, enumerate-on-start

SQLite FTS5 and `notify`-based file watching are correct long-term but
introduce complexity (schema migrations, watcher reliability across
network drives, debouncing) that this spec does not need to prove. A
`Vec<PathBuf>` built once at `atref run` startup is sufficient to
exercise the matcher, picker, and injection paths on real corpora.

### Clipboard-paste injection (with restore) over keystroke synthesis

`arboard` + simulated `Ctrl+V` is the simplest reliable path-insertion
method that works across all top-target apps (terminals, browsers,
Electron, Office). The IME-related complications of
character-by-character keystroke synthesis are real but not on the
critical path of proving the concept. Per **TC6**, the clipboard
restore is a known fragile point and is documented for the user.

### Cursor-position anchoring (no UIA)

UI Automation `TextPattern` does not return a caret rectangle for most
Electron apps without `--force-renderer-accessibility`. The strategic
vault note already accepted "near the caret, not on it" as the
default. v0.1 takes the simpler path: position at the mouse cursor.
Caret-rect anchoring with UIA is a future spec.

### `atref describe` as the self-description contract

Adopting the agent-mail-cli pattern: a single `describe` command
returns a structured JSON document covering the tool's purpose,
commands, config schema, and invariants. Agents (and humans) read it
to learn what `atref` can do without parsing `--help` output.

## Pre-requisites (Human Required)

- [ ] Rust toolchain installed (`rustup` with stable channel ≥ 1.75).
- [ ] On Windows: `cargo --version` runs in PowerShell or Windows
      Terminal without error.
- [ ] An indexed folder containing at least one regular file (e.g. the
      user's `Documents`, or `D:\jfuchs\dev\second-brain`).

## Implementation Tasks

- [ ] Scaffold a Rust workspace at the repo root with a single binary
      crate `atref` (`src/main.rs`).
- [ ] Add all dependencies listed in **TC3** to `Cargo.toml`.
- [ ] Implement the `clap`-derived CLI matching **FR1** (`run`,
      `describe`, `--version`, `--help`).
- [ ] Implement TOML config loading per **FR2** with first-launch
      default-write behavior.
- [ ] Implement recursive file enumeration per **FR3** (`walkdir` with
      exclude-glob pruning, hidden-file filtering).
- [ ] Implement chord registration per **FR4** via `global-hotkey`.
- [ ] Build the `eframe` picker window per **FR5**, **FR6**, **FR8**.
      Window is pre-warmed (created hidden at startup) and shown on
      chord per **NFR2**.
- [ ] Wire `nucleo-matcher` per **FR7** with `Config::DEFAULT.match_paths()`.
- [ ] Implement the previously-focused-window capture and
      restoration needed for **FR5**, **FR9**, **FR10** (Win32
      `GetForegroundWindow` + `SetForegroundWindow`).
- [ ] Implement clipboard-paste injection per **FR9** using `arboard`
      and `enigo`.
- [ ] Implement graceful shutdown per **FR11**.
- [ ] Implement `atref describe` JSON output per **FR1** and the Usage
      Examples below.
- [ ] Run the Testing Approach end-to-end against the user's real
      indexed folder.

## Acceptance Criteria

### CLI surface

- [ ] **AC1**: `atref --version` prints the version from `Cargo.toml`
      to stdout and exits 0.
- [ ] **AC2**: `atref --help` prints help listing `run` and
      `describe` and exits 0.
- [ ] **AC3**: `atref describe` prints a single JSON document to
      stdout (UTF-8, two-space-indented) that includes top-level keys
      `purpose`, `commands`, `config`, and `invariants`. Exits 0 even
      when no config file exists.
- [ ] **AC4**: `atref describe` runs successfully on a machine with no
      network access.
- [ ] **AC5**: Unknown subcommand prints an error to stderr and exits
      non-zero.

### Config loading

- [ ] **AC6**: On first launch when `%APPDATA%\atref\config.toml` does
      not exist, `atref run` creates the parent directory and writes a
      default config file there, then proceeds.
- [ ] **AC7**: A malformed TOML config causes `atref run` to print a
      readable error to stderr and exit non-zero (no partial state
      written, no chord registered).

### Indexing

- [ ] **AC8**: With the indexed root set to a folder containing at
      least 100 regular files in nested directories, `atref run`
      enumerates them within 1 second on warm filesystem cache and
      keeps them in memory.
- [ ] **AC9**: Hidden files and entries under `.git`, `node_modules`,
      `target` are excluded from the index.

### Chord

- [ ] **AC10**: Pressing `Ctrl+Space` while focused in Notepad, in
      PowerShell, and in a browser address bar (Chrome or Edge) shows
      the picker window within 100 ms of the keypress (**NFR2**).
- [ ] **AC11**: When the configured chord is changed in
      `config.toml` (e.g. to `Alt+Space`) and `atref run` is
      restarted, the new chord triggers the picker and the old one
      does not.

### Picker UI

- [ ] **AC12**: The picker appears with the text-input field
      focused and the cursor inside it, ready to receive typing
      without an extra click.
- [ ] **AC13**: With an empty filter, the picker shows up to 10 files
      from the index (any order acceptable for v0.1).
- [ ] **AC14**: Typing characters into the input updates the result
      list within one frame (≤ 16 ms target per **NFR1**).
- [ ] **AC15**: `↓` and `↑` arrow keys move the selection by one row
      and wrap at the ends.
- [ ] **AC16**: `Esc` closes the picker and returns focus to the
      previously-focused application. The clipboard is unchanged.

### Insertion

- [ ] **AC17**: With Notepad focused before chord press, pressing
      `Enter` on a selected file causes that file's absolute path
      (Windows-style with backslashes) to appear at the caret position
      in Notepad within 500 ms of the keypress.
- [ ] **AC18**: After insertion completes, the clipboard contains the
      same text it held before the chord was pressed (verified by
      pasting again after a 1-second wait).
- [ ] **AC19**: AC17 also passes with Obsidian, VSCode, the Chrome
      address bar, and Windows Terminal as the focused application.

### Shutdown

- [ ] **AC20**: `Ctrl+C` in the controlling terminal causes
      `atref run` to exit with code 0 within 1 second, with no
      orphaned picker window remaining.

## Testing Approach

### Validation Steps

1. **Build:** From the repo root, `cargo build --release`. Expected:
   exits 0; `target\release\atref.exe` exists and is < 20 MB.
2. **Lint:** `cargo clippy --all-targets -- -D warnings`. Expected:
   exits 0.
3. **Format:** `cargo fmt --check`. Expected: exits 0.
4. **CLI surface:** Run `atref --version`, `atref --help`,
   `atref describe`, `atref bogus`. Compare against AC1–AC5.
5. **Config bootstrap:** Delete `%APPDATA%\atref\config.toml`. Run
   `atref run`. Inspect the file that was written. Compare against
   AC6.
6. **Indexing:** Set the indexed root to a folder of ≥ 100 files
   including a `.git` subdirectory and a hidden file. Run
   `atref run`. Open the picker with chord, observe that hidden /
   `.git` files do not appear. Compare against AC8, AC9.
7. **Chord & picker (human-in-the-loop):** Run the picker against
   each focused-app from the AC10 / AC19 list. For each:
   1. Open the target app and place the caret in a text field.
   2. Press the configured chord.
   3. Observe the picker appears (AC10), input is focused (AC12),
      filter updates per keystroke (AC14), arrows navigate (AC15).
   4. Pick a file, press `Enter`, observe the absolute path appears at
      the caret (AC17, AC19).
   5. Copy a known string to the clipboard. Re-run the picker. After
      insertion completes, paste again — verify the original string
      reappears (AC18).
   6. Press the chord, press `Esc`, verify no insertion and original
      clipboard intact (AC16).
8. **Latency measurement:** With an indexed folder of ~10,000 files
   (e.g. a large repo's `.git`-excluded tree), time the first paint
   from chord press using OS-level video capture or a stopwatch
   approximation. Compare to NFR2.
9. **Shutdown:** While `atref run` is active, press `Ctrl+C` in the
   controlling terminal; confirm AC20 within 1 second.

### Test Cases

| Input | Expected Output |
|-------|-----------------|
| `atref --version` | Single line matching the `Cargo.toml` version; exit 0 |
| `atref --help` | Help text listing `run` and `describe`; exit 0 |
| `atref describe` | JSON object with keys `purpose`, `commands`, `config`, `invariants`; exit 0 |
| `atref describe` on a machine with no network | Same output as above; exit 0 |
| `atref run` with valid config | Picker shows on chord; quits on `Ctrl+C` exit 0 |
| `atref run` with malformed `config.toml` | Error on stderr; exit non-zero; no chord registered |
| Chord pressed while focused in Notepad | Picker appears within 100 ms |
| `Enter` on selected file in picker (Notepad focused) | Absolute path inserted at caret within 500 ms |
| `Esc` in the picker | Picker closes; clipboard unchanged; no insertion |
| Empty index | Picker shows "no files indexed" placeholder row (no crash) |

### Human-in-the-Loop Testing Protocol

The picker, chord, and injection paths cannot be verified purely
programmatically. The implementing agent must hand off to JJ for the
final acceptance pass:

1. **Agent:** Build the release binary and complete all automated
   checks (AC1–AC9, lint, format, build).
2. **Agent:** Pause and ask JJ to manually verify AC10–AC20 by
   walking through Validation Step 7 above against each target app
   (Notepad, PowerShell, Chrome, Edge, Obsidian, VSCode, Windows
   Terminal).
3. **JJ:** Executes the protocol, reports pass/fail per AC.
4. **Agent:** Iterate on any failures; re-run the protocol until
   every AC passes.

## Usage Examples

### CLI

```text
> atref --version
atref 0.1.0

> atref --help
A global file-reference picker.

Usage: atref <COMMAND>

Commands:
  run       Start the foreground process and listen for the chord
  describe  Print a JSON description of atref's commands and config

Options:
  -h, --help     Print help
  -V, --version  Print version

> atref describe
{
  "purpose": "Global file-reference picker triggered by a keyboard chord.",
  "commands": {
    "run": {
      "args": [],
      "behavior": "Loads config from %APPDATA%\\atref\\config.toml, enumerates the configured root folder, registers the configured chord, and shows the picker when the chord fires. Foreground process; Ctrl+C to quit."
    },
    "describe": {
      "args": [],
      "behavior": "Prints this document."
    }
  },
  "config": {
    "path": "%APPDATA%\\atref\\config.toml",
    "schema": {
      "indexed_root": "absolute path of the folder to index",
      "chord": "global-hotkey chord string, e.g. \"Control+Space\"",
      "exclude_globs": "list of directory globs to skip"
    },
    "defaults": {
      "indexed_root": "%USERPROFILE%",
      "chord": "Control+Space",
      "exclude_globs": [".git", "node_modules", "target"]
    }
  },
  "invariants": [
    "Single binary, foreground process.",
    "Index is built once at startup; file changes after that are not picked up until restart.",
    "Insertion mechanism is clipboard-paste with restore; collisions with other clipboard tools are possible.",
    "Picker appears at the mouse cursor, not at the text caret."
  ]
}
```

### Config file

```toml
# %APPDATA%\atref\config.toml

indexed_root = "D:\\jfuchs\\dev\\second-brain"
chord = "Control+Space"
exclude_globs = [".git", ".obsidian", "node_modules", "target"]
```

## Out of Scope

The following are deferred to later specs and must not appear in
v0.1's implementation:

- macOS and Linux ports.
- `@` keystroke as a trigger (event tap / `SetWindowsHookEx`).
- Multiple indexed folders / profiles.
- File-watcher-driven incremental index updates (`notify` crate).
- Persistent index (SQLite FTS5).
- Caret-rect anchoring via UI Automation.
- Output format cycling at selection time (relative path / wikilink /
  custom template). v0.1 inserts the absolute path only.
- Match-position highlighting in the result list.
- Frecency / recently-selected boost.
- Daemonization, Windows service, autostart.
- Installer, signed executable, `winget` manifest, `brew` formula,
  `npx` wrapper, `cargo install` publishing.
- Tauri (chosen for a future spec; v0.1 uses `egui`).
- Synthesized-keystroke insertion (clipboard-paste only for v0.1).
- Logging, telemetry, crash reporting.
- Configuration UI; config is edited by hand in TOML.

## References

- Strategic project node: `D:\jfuchs\dev\second-brain\📦 atref.md`
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`
- Reference implementations to study (per the project node's
  GitHub-scan findings):
  - [`espanso/espanso`](https://github.com/espanso/espanso) — global
    keyboard hook patterns (GPLv3 — borrow patterns, not code).
  - [`pepperonas/inspector-rust`](https://github.com/pepperonas/inspector-rust)
    — Tauri 2 + Rust + global hotkey + AX/UIA paste fallback.
  - [`autobib/nucleo-picker`](https://github.com/autobib/nucleo-picker)
    — examples of `nucleo`'s event-driven streaming API.
- Crate docs:
  - [`global-hotkey`](https://docs.rs/global-hotkey/)
  - [`nucleo-matcher`](https://docs.rs/nucleo-matcher/)
  - [`eframe`](https://docs.rs/eframe/) / [`egui`](https://docs.rs/egui/)
  - [`arboard`](https://docs.rs/arboard/)
  - [`enigo`](https://docs.rs/enigo/)
  - [`walkdir`](https://docs.rs/walkdir/)

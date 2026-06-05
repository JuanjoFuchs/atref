---
id: "001"
title: atref Windows MVP — tray app with chord-triggered picker and @-quoted path insertion
status: complete
blocked_by: []
blocks: []
---

# atref Windows MVP

## Overview

This spec defines the behavioral contract for the first runnable version
of `atref` on Windows: a **system-tray application**. Launching
`atref.exe` (by double-click or from a shell) places an icon in the
Windows notification area with **no console window**, loads a JSON config,
builds an in-memory file index over one or more configured folders, and
registers a global chord (default `Ctrl+Space`). Pressing the chord while
focused in any application shows a borderless fuzzy picker near the mouse
cursor; the user types to filter, and on `Enter` the selected file is
inserted at the caret of the previously-focused application as an
`@`-prefixed, always-double-quoted absolute path — e.g.
`@"D:\jfuchs\dev\second-brain\📦 atref.md"`. Right-clicking the tray icon
exposes *Open config file*, *Reload config*, and *Quit*. All
configuration is done by editing the JSON file by hand.

This spec is intentionally narrow. It does **not** cover: a settings GUI,
autostart-with-Windows, insertion-format variants beyond `Enter`
(wikilink / relative / Markdown via Ctrl/Shift/Alt), the `@` keystroke
trigger, macOS or Linux, a persistent SQLite/FTS5 index, file-watcher
incremental updates, caret-rect anchoring via UI Automation, match-position
highlighting, frecency, or `winget` / `brew` / `npx` distribution. Each is
a follow-up spec.

> **Completion rule:** This spec is not complete until every Acceptance
> Criterion is verified through the Testing Approach below, including the
> human-in-the-loop manual tests. Build-only verification is insufficient.
> The agent must iterate until verification passes.

## Goals

- Prove that the four hard primitives — chord registration, picker window,
  fuzzy filtering, caret insertion — work together on Windows, hosted
  inside a persistent **tray application** with no terminal required.
- Establish a Rust workspace and crate dependency baseline future specs
  build on, including the tray-icon + GUI event-loop integration.
- Produce a single static `atref.exe` that runs as a tray app and
  demonstrates the workflow end-to-end without an installer.
- Pick tray, UI, hotkey, matcher, and injection libraries deliberately so
  later specs can extend (settings GUI, multi-OS, format cycling,
  distribution) without rewriting the core.

## Requirements

### Functional Requirements

- **FR1 — Tray launch, no console.** Running `atref.exe` starts a
  resident GUI process with no console window (release builds use
  `#![windows_subsystem = "windows"]`) and adds an icon to the Windows
  notification area. The process stays alive until *Quit*.
- **FR2 — Tray menu.** Right-clicking the tray icon shows a menu
  containing at least: a disabled label `atref v<version>`; **Open config
  file** (opens `config.json` in the OS default handler); **Reload
  config** (re-reads `config.json`, rebuilds the index, re-registers the
  chord); **Quit** (unregisters the chord, removes the tray icon, destroys
  the picker window, exits 0).
- **FR3 — JSON configuration.** Configuration is read from
  `%APPDATA%\atref\config.json`. If the file does not exist on launch,
  atref creates the parent directory and writes a default config (see
  schema below), then proceeds. Schema:

  ```json
  {
    "folders": ["D:\\jfuchs\\dev\\second-brain", "D:\\jfuchs\\dev"],
    "chord": "Control+Space",
    "exclude": [".git", "node_modules", "target"]
  }
  ```

  `folders` is a non-empty array of absolute directory paths. `chord` uses
  [`HotKey::from_str`](https://docs.rs/global-hotkey/) syntax (e.g.
  `"Control+Space"`, `"Alt+Shift+P"`). `exclude` is a list of directory
  names pruned during traversal. Defaults: `folders = [user home]`,
  `chord = "Control+Space"`, `exclude = [".git", "node_modules",
  "target"]`.
- **FR4 — Config errors without a console.** Because there is no console,
  a missing-but-uncreatable, unreadable, malformed, or schema-invalid
  (e.g. empty `folders`) `config.json` surfaces as a **native error
  dialog** describing the problem. On **launch** with no usable config,
  atref shows the dialog and exits non-zero without registering the chord.
  On **Reload config** with a bad file, atref shows the dialog and keeps
  running on the last-good in-memory config.
- **FR5 — Indexing.** On launch and on every *Reload config*, recursively
  enumerate every regular file under each configured folder into an
  in-memory index of `{ absolute_path, root }`. Symlinks are not followed.
  Hidden files (dot-prefixed, or with the Windows hidden attribute) are
  skipped. Directories whose name matches an `exclude` entry are pruned
  during traversal. The same absolute path is indexed once even if it
  falls under more than one configured (nested) root.
- **FR6 — Chord registration.** Register a single global keyboard chord
  via the `global-hotkey` crate. Default `Ctrl+Space`, configurable via
  `chord`. The chord is re-registered on *Reload config*.
- **FR7 — Picker window on chord.** When the chord fires, show a
  borderless, always-on-top picker window positioned with its top-left
  corner offset 12 px right and 24 px below the current mouse-cursor
  position. The window appears in front of the previously-focused
  application; that application's window handle is captured so insertion
  can target it. The window is pre-allocated and kept hidden between
  activations to meet the first-paint budget (**NFR2**).
- **FR8 — Picker contents.** The picker contains, top-to-bottom: a
  single-line text input with the caret already in it, and a list of up to
  10 matching files. Each row shows the basename in normal weight and the
  parent directory (relative to that file's root) in a dimmer color. The
  selected row is highlighted. When the index is empty, a single
  "no files indexed" placeholder row is shown (no crash).
- **FR9 — Filtering.** As the user types, the result list updates per
  keystroke. Matching is performed by `nucleo-matcher` against each file's
  path-relative-to-its-root, using its path-aware matching configuration
  (basename-weighted, smart case, CamelCase aware).
- **FR10 — Picker keyboard model.**
  - `↓` / `↑` move the selection one row and wrap at the ends.
  - `Enter` accepts the selected row.
  - `Esc` dismisses the picker with no insertion.
  - Any other character or `Backspace` edits the filter input.
  - `Ctrl`/`Shift`/`Alt` + `Enter` are reserved for the format-cycling
    spec and are **not** handled in v0.1.
- **FR11 — Insertion on `Enter`.** The picker hides immediately, then
  atref inserts the string `@"<ABS>"` — a literal `@`, followed by the
  selected file's Windows-style absolute path wrapped in double quotes
  (always quoted, even when the path contains no spaces) — at the caret of
  the previously-focused application by: (a) reading and saving the
  current Windows clipboard text, (b) writing the `@"<ABS>"` string to the
  clipboard, (c) restoring focus to the previously-focused window,
  (d) synthesizing a `Ctrl+V` keystroke, (e) waiting 150 ms, (f) restoring
  the original clipboard text.
- **FR12 — `Esc`.** No insertion occurs, the clipboard is not modified,
  and focus returns to the previously-focused window.
- **FR13 — Quit.** *Quit* from the tray menu unregisters the hotkey,
  destroys the picker window, removes the tray icon, and exits with code 0.

### Non-Functional Requirements

- **NFR1**: Per-keystroke filter latency from key event to repaint is
  under 16 ms on an index of ≤ 10,000 files on a modern desktop.
- **NFR2**: From chord press to picker first paint is under 100 ms on the
  same hardware, achieved by keeping the picker window pre-allocated and
  hidden between activations.
- **NFR3**: Idle memory footprint (tray running, picker hidden) is under
  250 MB resident. The egui/glow window is resident in v0.1, so this reflects
  the GL-context + font-atlas baseline (~157 MB measured 2026-06-04); the
  original sub-60 MB goal returns with the future daemon + sidecar
  architecture, where the picker window is spawned on demand rather than kept
  resident.
- **NFR4**: `atref.exe` is a single statically-linked binary. No external
  runtime (no `.NET`, no `WebView2`, no Python) is required.
- **NFR5**: Launching the release binary shows **no console window** and
  no visible main window — only the tray icon.

### Technical Constraints

- **TC1**: Rust, edition 2021, stable toolchain ≥ 1.75.
- **TC2**: Windows 10 (build 19041+) and Windows 11, x64 only.
- **TC3**: Dependency choices (locked by this spec; alternatives require an
  amendment):
  - GUI framework: `eframe` / `egui` ≥ 0.27
  - Tray icon + menu: `tray-icon` ≥ 0.14 (Tauri-maintained; pairs with
    `global-hotkey`)
  - Global hotkeys: `global-hotkey` ≥ 0.5
  - Fuzzy matcher: `nucleo-matcher` ≥ 0.3
  - Clipboard: `arboard` ≥ 3.4
  - Synthesized keystrokes: `enigo` ≥ 0.3
  - File enumeration: `walkdir` ≥ 2
  - Config / serialization: `serde` ≥ 1 + `serde_json` ≥ 1 (**JSON, not
    TOML**)
  - Config-directory resolution: `directories` ≥ 5
  - Native error dialogs: `rfd` ≥ 0.14 (message boxes; there is no console
    to print to)
  - **No `clap` and no CLI subcommands** — launch is the GUI.
- **TC4**: Single process. No background service and **no
  autostart-with-Windows** in this spec (the app is tray-resident only
  while running; launch-at-login is a follow-up).
- **TC5**: No installer, no `winget` manifest, no signed executable.
  `cargo build --release` produces `target\release\atref.exe`; the user
  runs it directly.
- **TC6**: The clipboard-paste injection is brittle: if another clipboard
  tool overwrites the clipboard inside the 150 ms restore window, the
  original contents are lost. Accepted for v0.1 and documented in the
  README. Synthesized keystrokes for arbitrary paths are deferred (they
  require IME-aware character composition).
- **TC7**: Event-loop integration is the principal v0.1 risk, **validated
  by spike on 2026-06-04** (see the off-screen-parking decision below).
  `eframe` owns the event loop, while `tray-icon` and `global-hotkey` each
  deliver events through their own global channels; those events are
  consumed within egui's update cycle, woken by a background thread. The
  picker window must be parked off-screen rather than hidden — a hidden
  eframe window stops being serviced and the loop stalls.

## Key Decisions

### Tray application, not a terminal process

The product's "is it running?" signal is a tray icon; a console window is
noise. v0.1 ships as a tray-resident GUI process: launch adds the icon,
*Quit* removes it. This replaces the earlier "foreground process,
`Ctrl+C` to quit" decision. The app does **not** auto-start with Windows
in this spec — that polish is a follow-up. The main cost of this decision
is the `eframe` + `tray-icon` + `global-hotkey` event-loop integration
(**TC7**), which is validated first.

### JSON config, hand-edited, no settings UI

Configuration lives in `%APPDATA%\atref\config.json` and is edited by
hand; *Reload config* re-reads it live. JSON is chosen over TOML by the
product owner. A graphical settings editor (add/remove folders, capture
the chord by pressing it) is deferred to the next spec; the JSON file is
the configuration contract for v0.1.

### `Enter`-only, `@`-quoted absolute path

The single insertion format for v0.1 is `@"<absolute path>"` — the `@`
sigil plus the full Windows path, always double-quoted. The
modifier-driven variants (`Ctrl+Enter` wikilink, `Shift+Enter` /
`Alt+Enter` other formats) are real product features but are deferred to
the format-cycling spec so v0.1 proves one insertion path cleanly.

### `egui`/`eframe` over Tauri for v0.1

Pure Rust, no Node/npm in the build, no `WebView2` runtime, smaller binary
(~6 MB vs ~12 MB), faster iteration. Tauri remains the candidate for a
later spec if richer styling or auto-updates are needed. Switching is a
single-crate concern, explicitly inside a future spec's scope.

### In-memory index, enumerate-on-start

A `Vec` built once at launch (and on *Reload config*) is sufficient to
exercise the matcher, picker, and injection paths on real corpora. SQLite
FTS5 and `notify`-based watching are correct long-term but add complexity
this spec does not need to prove.

### Clipboard-paste injection (with restore)

`arboard` + simulated `Ctrl+V` is the simplest reliable insertion method
that works across terminals, browsers, Electron, and Office. The IME
complications of keystroke synthesis are real but off the critical path
for proving the concept. The clipboard restore is a known fragile point
(**TC6**), documented for the user.

### Cursor-position anchoring (no UIA)

UI Automation `TextPattern` does not return a caret rectangle for most
Electron apps without `--force-renderer-accessibility`. v0.1 positions the
picker at the mouse cursor ("near the caret, not on it"). Caret-rect
anchoring via UIA is a future spec.

### No CLI, no `describe`

atref is an end-user desktop application distributed to consumers, not a
vault script that AI agents introspect. There are therefore **no
subcommands** and no `describe` JSON contract (that pattern came from the
vault's agent tooling and does not apply here). Launching the binary opens
the GUI; the version is shown as a tray-menu label. With the GUI subsystem
there is no console for `--version` / `--help` to print to.

### Show/hide by off-screen parking, not window hiding (validated 2026-06-04)

A throwaway spike (`atref-spike-tc7`) proved the eframe + tray-icon +
global-hotkey integration and surfaced one binding constraint: the picker
window must **never be hidden via window-visibility**. When the eframe
window is hidden, winit stops servicing it and a cross-thread
`request_repaint()` wake becomes unreliable — the loop stalls after a
couple of show/hide cycles and drops both chord and tray events. Keeping
the window *visible but parked off-screen* (with no taskbar entry) keeps
the loop reliably serviced: "show" moves it on-screen and focuses it,
"hide" moves it off-screen. With that change, repeated chord show/hide
cycles and tray *Quit* were all reliable. This rule is binding on the
**FR7**/**FR8** implementation.

## Pre-requisites (Human Required)

- [x] Rust toolchain installed (`rustup`, stable channel ≥ 1.75).
- [x] `cargo --version` runs in PowerShell or Windows Terminal.
- [x] At least one folder containing ≥ 1 regular file to index (e.g.
      `D:\jfuchs\dev\second-brain`).

## Implementation Tasks

- [x] Replace the placeholder `src/main.rs` with the real binary crate;
      add all **TC3** dependencies to `Cargo.toml`.
- [x] Build the release binary as a GUI-subsystem app so no console window
      appears on launch (a console in debug builds for logs is fine).
- [x] Implement JSON config load + schema validation per **FR3**, the
      first-launch default-write, and the *Reload config* path; surface
      errors via `rfd` dialogs per **FR4**.
- [x] Implement recursive enumeration per **FR5** (`walkdir`, exclude-name
      pruning, hidden-file filtering, multi-root dedupe).
- [x] Add the tray icon and menu per **FR1**/**FR2** (`tray-icon`), wired
      to *Open config file*, *Reload config*, *Quit*.
- [x] Register the chord per **FR6** (`global-hotkey`), re-registered on
      reload.
- [x] Build the `eframe` picker per **FR7**/**FR8**/**FR10**; pre-warm it
      hidden at startup and show it on chord; consume tray and hotkey
      events within egui's update cycle per **TC7**.
- [x] Wire `nucleo-matcher` per **FR9** using its path-aware matching
      configuration.
- [x] Implement previously-focused-window capture and restoration via the
      Win32 foreground-window APIs for **FR7**/**FR11**/**FR12**.
- [x] Implement clipboard-paste insertion per **FR11** (`arboard` +
      `enigo`), inserting the `@"<ABS>"` string.
- [x] Implement *Quit* / graceful shutdown per **FR13**.
- [x] Run the Testing Approach end-to-end against the user's real folders.

## Acceptance Criteria

### Launch & tray

- [x] **AC1**: Launching the release `atref.exe` adds a tray icon and
      shows **no console window** and no main window.
- [x] **AC2**: Right-clicking the tray icon shows a menu containing a
      version label, *Open config file*, *Reload config*, and *Quit*.
- [x] **AC3**: *Open config file* opens `%APPDATA%\atref\config.json` in
      the OS default handler.
- [x] **AC4**: *Quit* removes the tray icon, unregisters the chord (the
      picker no longer appears on the chord), exits with code 0, and leaves
      no orphaned window.

### Config

- [x] **AC5**: On first launch with no `config.json`, atref creates
      `%APPDATA%\atref\config.json` with defaults, then runs.
- [x] **AC6**: A malformed or schema-invalid `config.json` causes a native
      error dialog: on launch atref exits without registering the chord; on
      *Reload config* atref keeps running on the last-good config (per
      **FR4**).
- [x] **AC7**: Editing `folders` or `chord` in `config.json` and choosing
      *Reload config* applies the change without restarting the process
      (the new chord triggers and the old one does not; files from a newly
      added folder become searchable).

### Indexing

- [x] **AC8**: With ≥ 100 regular files across nested directories in the
      configured folders, indexing completes within ~1 second on warm
      filesystem cache and the files stay in memory.
- [x] **AC9**: Hidden files and entries under `.git`, `node_modules`, and
      `target` are excluded from the index.
- [x] **AC10**: With multiple folders configured, files from all of them
      are searchable in the picker.

### Chord & picker

- [x] **AC11**: Pressing the chord while focused in Notepad, in PowerShell,
      and in a browser address bar (Chrome or Edge) shows the picker within
      100 ms (**NFR2**).
- [x] **AC12**: The picker opens with the text input focused and the cursor
      inside it, ready to type without an extra click.
- [x] **AC13**: With an empty filter the picker shows up to 10 files; with
      an empty index it shows the "no files indexed" placeholder (no crash).
- [x] **AC14**: Typing updates the result list within one frame (≤ 16 ms
      target per **NFR1**).
- [x] **AC15**: `↓` / `↑` move the selection by one row and wrap at the
      ends.
- [x] **AC16**: `Esc` closes the picker and returns focus to the
      previously-focused application; the clipboard is unchanged.

### Insertion

- [x] **AC17**: With Notepad focused before the chord, pressing `Enter` on
      a selected file inserts `@"<ABS>"` — a literal `@` plus the
      double-quoted Windows-style absolute path — at the caret within
      500 ms.
- [x] **AC18**: After insertion, the clipboard holds the same text it held
      before the chord (verified by pasting again after a 1-second wait).
- [x] **AC19**: AC17 also passes with Obsidian, VSCode, the Chrome address
      bar, and Windows Terminal as the focused application.

### Performance & footprint

- [x] **AC20**: With the picker hidden, the running process stays under
      250 MB resident memory (**NFR3**), observed in Task Manager.

## Testing Approach

### Validation Steps

1. **Build:** `cargo build --release`. Expected: exits 0;
   `target\release\atref.exe` exists and is < 20 MB.
2. **Lint:** `cargo clippy --all-targets -- -D warnings`. Expected: 0.
3. **Format:** `cargo fmt --check`. Expected: 0.
4. **Launch & tray:** Double-click the release binary. Confirm a tray icon
   appears with no console and no window (AC1). Exercise each menu item
   (AC2–AC4).
5. **Config bootstrap:** Delete `%APPDATA%\atref\config.json`; launch;
   inspect the written file (AC5). Corrupt it; launch and *Reload config*;
   confirm the error dialog and that no chord is registered (AC6). Edit
   `folders`/`chord` and *Reload config* (AC7).
6. **Indexing:** Configure two folders totalling ≥ 100 files including a
   `.git` subdirectory and a hidden file. Launch, open the picker, confirm
   hidden / `.git` files are absent and files from both folders appear
   (AC8–AC10).
7. **Chord, picker & insertion (human-in-the-loop):** For each target app
   (Notepad, PowerShell, Chrome/Edge address bar, Obsidian, VSCode, Windows
   Terminal):
   1. Place the caret in a text field.
   2. Press the configured chord; observe the picker appears (AC11), input
      is focused (AC12), the list filters per keystroke (AC14), arrows
      navigate and wrap (AC15).
   3. Pick a file, press `Enter`, observe `@"<ABS>"` appears at the caret
      (AC17, AC19).
   4. Copy a known string to the clipboard, re-run the picker, insert, then
      paste again — verify the original string reappears (AC18).
   5. Press the chord, press `Esc`, verify no insertion and the clipboard
      is intact (AC16).
8. **Latency:** With an indexed folder of ~10,000 files, measure first
   paint from chord press (OS video capture or stopwatch) against NFR2.
9. **Footprint:** With atref running and the picker hidden, check the
   process's resident memory in Task Manager against NFR3 (< 60 MB).

### Test Cases

| Input | Expected Output |
|-------|-----------------|
| Launch release `atref.exe` | Tray icon appears; no console, no window |
| Right-click tray icon | Menu: version label, Open config file, Reload config, Quit |
| First launch, no `config.json` | Default `config.json` written; app runs |
| Malformed `config.json` | Native error dialog; no chord registered |
| Edit config + *Reload config* | New chord/folders take effect, no restart |
| Chord pressed while focused in Notepad | Picker appears within 100 ms |
| `Enter` on selected file (Notepad focused) | `@"<ABS>"` inserted at caret within 500 ms |
| `Esc` in the picker | Picker closes; clipboard unchanged; no insertion |
| Empty index | Picker shows "no files indexed" row (no crash) |
| *Quit* from tray menu | Tray icon gone; chord no longer fires; exit 0 |

### Human-in-the-Loop Testing Protocol

The picker, chord, and injection paths cannot be verified purely
programmatically. The implementing agent must hand off to JJ for the final
acceptance pass:

1. **Agent:** Build the release binary and complete all automated checks
   (AC1–AC10, lint, format, build).
2. **Agent:** Pause and ask JJ to manually verify AC11–AC19 by walking
   through Validation Step 7 against each target app.
3. **JJ:** Executes the protocol, reports pass/fail per AC.
4. **Agent:** Iterate on any failures; re-run until every AC passes.

## Usage Examples

### Launch

Double-click `atref.exe` (or run `.\target\release\atref.exe`). A tray
icon appears; there is no console and no window. Right-click the icon for
the menu.

### Config file

```json
// %APPDATA%\atref\config.json
{
  "folders": ["D:\\jfuchs\\dev\\second-brain", "D:\\jfuchs\\dev"],
  "chord": "Control+Space",
  "exclude": [".git", ".obsidian", "node_modules", "target"]
}
```

### Insertion

Focused in Obsidian, press the chord, type `atref`, press `Enter`. Inserted
at the caret:

```text
@"D:\jfuchs\dev\second-brain\📦 atref.md"
```

## Out of Scope

The following are deferred to later specs and must not appear in v0.1:

- Graphical settings UI (add/remove folders, click-to-set chord).
- Autostart-with-Windows / Windows service.
- Insertion-format variants: `Ctrl+Enter` (wikilink), `Shift+Enter`,
  `Alt+Enter` (other formats). v0.1 inserts `@"<ABS>"` on `Enter` only.
- `@` keystroke as a trigger (event tap / `SetWindowsHookEx`).
- macOS and Linux ports.
- File-watcher-driven incremental index updates (`notify`).
- Persistent index (SQLite FTS5).
- Caret-rect anchoring via UI Automation.
- Match-position highlighting; frecency / recently-selected boost.
- Synthesized-keystroke insertion (clipboard-paste only for v0.1).
- Installer, signed executable, `winget` / `brew` / `npx` /
  `cargo install` publishing.
- Tauri (chosen for a future spec; v0.1 uses `egui`).
- Logging, telemetry, crash reporting.
- CLI subcommands and a `describe` self-description contract.

## References

- Strategic project node: `D:\jfuchs\dev\second-brain\📦 atref.md`
- Spec rules: `D:\jfuchs\dev\second-brain\Spec Writing Rules for Agents.md`
- Reference implementations to study (per the project node's GitHub scan):
  - [`espanso/espanso`](https://github.com/espanso/espanso) — global
    keyboard hook patterns (GPLv3 — borrow patterns, not code).
  - [`pepperonas/inspector-rust`](https://github.com/pepperonas/inspector-rust)
    — Rust + global hotkey + tray + AX/UIA paste fallback.
  - [`autobib/nucleo-picker`](https://github.com/autobib/nucleo-picker) —
    examples of `nucleo`'s event-driven streaming API.
- Crate docs:
  - [`global-hotkey`](https://docs.rs/global-hotkey/) /
    [`tray-icon`](https://docs.rs/tray-icon/)
  - [`nucleo-matcher`](https://docs.rs/nucleo-matcher/)
  - [`eframe`](https://docs.rs/eframe/) / [`egui`](https://docs.rs/egui/)
  - [`arboard`](https://docs.rs/arboard/) / [`enigo`](https://docs.rs/enigo/)
  - [`walkdir`](https://docs.rs/walkdir/) / [`rfd`](https://docs.rs/rfd/) /
    [`serde_json`](https://docs.rs/serde_json/)

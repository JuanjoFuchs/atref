# Changelog

All notable changes to atref will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Match-position highlighting** — result rows emphasize the exact characters
  the fuzzy matcher matched (accent-colored, in both name and path), straight
  from the ranker's own match indices so what's bolded is exactly what was
  scored. (spec 009)
- **Per-result file metrics** — every row shows its size, and visible rows fill
  in line count and an estimated token count (`3.2 KB · 95 ln · ~1.1k tok`,
  tiktoken `o200k_base`) so you can see what a reference will cost in context
  before inserting it. Lazy, cached, and computed off the UI thread. (spec 010)
- **Image thumbnails** — png/jpg/gif/webp/svg results show a small thumbnail at
  the row's right edge (GIFs use their first frame, SVGs render natively).
  (spec 011)

### Fixed
- **Cloud placeholders are never downloaded** — metrics and thumbnails stop at
  filesystem metadata for OneDrive/Dropbox cloud-only files
  (`FILE_ATTRIBUTE_OFFLINE` / `RECALL_ON_DATA_ACCESS`), so browsing results
  can't trigger hydration.
- **Tray menu no longer closes instantly while the picker is open** — opening
  the tray menu foregrounds the tray's own window, which blurred the picker;
  the picker's blur-hide then yanked the foreground back to the pre-summon app,
  dismissing the menu in a flash. Blur-dismissal now leaves focus where the
  user put it; only explicit dismissal (`Esc` / ✕) restores focus.

## [0.5.1] - 2026-06-09

### Added
- **Start Menu shortcut** — the PowerShell installer (`install.ps1`) now creates a
  per-user Start Menu shortcut, so atref is searchable from Start after an
  `irm | iex` install (not just runnable from a new terminal). Matches Scoop.

### Changed
- **`folders` may be empty** — an empty `folders` list is now a valid blank-slate
  state (the picker indexes nothing) rather than a validation error, and
  `atref config remove folders <last>` succeeds instead of being refused. Lets an
  agent — or a human mid-swap — clear folders and add them back. (spec 007)

## [0.5.0] - 2026-06-08

First public release. Windows system-tray file-reference picker.

### Added
- **Picker** — global chord (default `Ctrl+Space`) summons a borderless,
  cursor-anchored fuzzy picker over the focused app; `Enter` inserts
  `@"<absolute path>"` at the caret, `Esc`/blur dismisses. (specs 001, 003, 004)
- **Tray app** — resident, no console window; right-click to open/reload config
  or quit.
- **Fuzzy matching** — `nucleo-matcher`, basename-weighted, smart-case,
  CamelHumps/initialism; match-position-aware ranking, source folder per result.
- **Result quality** — multi-folder indexing, manual excludes, folder-priority
  ranking, git-aware indexing (follow `.gitignore`), and a live file-watcher that
  picks up new/changed files. (spec 002)
- **Persistent index + frecency** — on-disk redb cache for instant launch with a
  background reconcile; an empty query leads with recent/most-used files and good
  matches get a bounded boost. (spec 005)
- **Config hot-reload** — edits to `config.json` (by hand or the CLI) apply to the
  running app without a manual reload. (spec 006)
- **Agent config CLI** — `atref describe` / `atref config get|set|add|remove` /
  `atref add` fully configure atref from the same binary; validated, atomic, JSON
  output. (spec 007)
- **GUI engine** — Tauri 2 / WebView2 (transparent borderless window with native
  acrylic), replacing the v0.1–v0.4 egui prototype.
- **Distribution** — GitHub Release `atref.exe` + WinGet (`JuanjoFuchs.atref`).
  (spec 008)

### Notes
- JSON config at `%APPDATA%\atref\config.json` (the early TOML plan was dropped in
  the spec-001 redefinition).
- Windows-only; macOS/Linux are a future port.

[Unreleased]: https://github.com/JuanjoFuchs/atref/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/JuanjoFuchs/atref/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/JuanjoFuchs/atref/releases/tag/v0.5.0

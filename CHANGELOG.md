# Changelog

All notable changes to atref will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/JuanjoFuchs/atref/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/JuanjoFuchs/atref/releases/tag/v0.5.0

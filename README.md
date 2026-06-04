# atref

A global file-reference picker. Press a keyboard chord anywhere in Windows
— terminal, browser, Obsidian, chat app, IDE — and an unobtrusive fuzzy
picker pops up near the caret. Type to filter, hit Enter, and a reference
to the selected file (an absolute path, a relative path, or an Obsidian
`[[wikilink]]`) is inserted right where you were typing.

The pretty name is **@ref**. Cross-platform is the goal; Windows is where
v0.1 lands.

## Status

**Pre-alpha. Nothing is shipped yet.** This repository is the destination
for atref's specs and implementation. Spec 001 is the behavioral
specification for the Windows MVP — chord trigger, single configured
folder, absolute-path insertion. Subsequent specs will add: file-watcher
indexing, `@`-trigger via OS event taps, macOS and Linux ports, format
cycling at selection time, daemon/auto-start, frecency, and distribution
via `winget` / `brew` / `npx`.

Tracking note in JJ's vault: `📦 atref.md`.

## Why

Claude Code's `@` file picker is excellent. It does not exist outside Claude
Code. atref is "that, but everywhere": cross-vault, cross-project,
cross-application, on the OS rather than inside one editor.

The hard parts (global keyboard hook, accessibility-aware text injection,
caret anchoring) are solved problems in `espanso` and `inspector-rust`.
atref's novel slice is: pre-indexed multi-folder file picker + caret-anchored
fuzzy UI + configurable reference formats.

## Planned installation

Once spec 001 has shipped and spec 002 (packaging) is complete:

```powershell
winget install atref
```

Other channels planned for later specs: `brew install atref` (macOS),
`apt install atref` (Linux), `cargo install atref`, `npx atref`.

## Repository layout

```
atref/
├── README.md
├── AGENTS.md            # Instructions for AI agents working in this repo
├── CLAUDE.md            # Points to AGENTS.md
├── CHANGELOG.md
├── LICENSE              # MIT
├── specs/
│   └── 001-windows-mvp.md
└── src/                 # Populated by spec 001 implementation
```

## License

MIT — see [LICENSE](LICENSE).

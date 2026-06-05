# AGENTS.md

atref is a Windows system-tray global file-reference picker (Rust + egui): press a
chord anywhere → a fuzzy picker → insert an `@"absolute path"` at the caret. Spec 001
(the v0.1 MVP) is **implemented and verified**; specs 002–003 are drafted next.

**CRITICAL: You MUST read the required files BEFORE taking action.** This is not optional.

## Required Reading by Task

| User asks about... | READ THIS FIRST | Then act |
|---|---|---|
| Understanding the project | This file, `README.md`, `📦 atref.md` (JJ's vault) | Explain or explore |
| What atref does today | `specs/001-windows-mvp.md` + `src/` | Answer using both |
| Drafting OR implementing ANY spec | `ai-docs/testable-architecture.md`, then the spec | Follow the testable seams; make every AC code-verifiable |
| Result quality (index/rank/watch) | `ai-docs/testable-architecture.md`, `specs/002-result-quality.md` | Spec → code behind seams → tests → ACs |
| Picker look & feel | `ai-docs/testable-architecture.md`, `specs/003-picker-look-and-feel.md` | Spec → code → kittest snapshot/input tests |
| Modifying behavior | The relevant spec + `src/` | Update spec first, then patch source |
| Updating agent instructions | this file | Edit this index; keep it ~50 lines |

**Do not skip this step.** Read the linked file first, then act.

## Architecture

```text
src/
├── lib.rs    # testable core: config, index, reference (+ picker state)
└── main.rs   # GUI shell: tray + global-hotkey + eframe picker + Win32 insertion
```

Single tray binary `atref.exe` (no daemon, no installer in v0.1). The picker window is
parked **off-screen, never hidden** — a hidden eframe window stalls the event loop
(spec 001, TC7). Keep behavior in the lib core; `main.rs` stays a thin view/wiring layer.

## Conventions

- Rust stable. `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` must pass before work is "done".
- **Testability is mandatory.** Every AC is validated by code where possible (`unit` / `integration` / `egui_kittest` input + snapshot); `manual` only for the irreducible OS sliver. See `ai-docs/testable-architecture.md`.
- Config: **JSON** at `%APPDATA%\atref\config.json`. No CLI subcommands, no env overrides.
- Spec rules: `D:/jfuchs/dev/second-brain/Spec Writing Rules for Agents.md`. Every AC names its validation method; no "Future Considerations" / "Success Criteria".
- Commits: do not commit unless JJ explicitly asks.

## Workflow

1. **READ** — routing table → `ai-docs/testable-architecture.md` → the spec.
2. **PLAN** — state the approach; flag scope creep vs the spec's Out of Scope.
3. **IMPLEMENT** — spec change first when behavior changes; code behind a testable seam.
4. **VERIFY** — run unit/integration/kittest tests for each AC; only the irreducible sliver is manual.

## Current State

- Spec 001 (Windows tray MVP) — **implemented + verified**, status complete. Release binary ~5.5 MB.
- Specs 002 (result quality) and 003 (picker look & feel) — drafted, status pending.
- Testing approach proven via the spike `D:\jfuchs\dev\atref-spike-testability` (kittest + Win32 insertion).
- Strategic node + roadmap dashboard: `📦 atref.md` in JJ's second-brain vault.

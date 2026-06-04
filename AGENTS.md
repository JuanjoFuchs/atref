# AGENTS.md

atref is a cross-platform global file-reference picker. Spec 001 is the
behavioral contract for the Windows MVP. The repository is currently
specs-only — there is no source code yet. The implementing agent reads
spec 001, scaffolds the Rust workspace per the spec's Implementation
Tasks, and verifies against the Acceptance Criteria.

**CRITICAL: You MUST read the required files BEFORE taking action.** This
is not optional.

## Required Reading by Task

| User asks about... | READ THIS FIRST | Then act |
|---|---|---|
| Understanding the project | This file, `README.md`, `specs/001-windows-mvp.md` | Explain or explore |
| What atref does today | `specs/001-windows-mvp.md`, source under `src/` once it exists | Answer using both |
| Implementing the MVP | `specs/001-windows-mvp.md` | Scaffold `Cargo.toml`, write code, verify against ACs |
| Modifying behavior | The relevant spec | Update spec first, then patch source |
| Adding or changing a flag | `specs/001-windows-mvp.md` (or the relevant spec) | Spec, then code, then ACs |
| Distribution (winget / brew / npx) | Future spec 002 (packaging) | Not yet written |
| Updating agent instructions | this file | Edit this index, keep it ~50 lines |

**Do not skip this step.** Read the linked file first, then act.

## Architecture (target — does not yet exist)

```text
src/                      # Rust workspace, populated by spec 001
└── atref/                # Single binary crate for v0.1
    ├── main.rs
    └── …                 # Internal modules — implementer's choice
```

Single binary `atref.exe` for v0.1 — no daemon, no installer.

## Conventions

- Rust 1.75+ stable. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must pass before any commit.
- JSON on stdout for non-interactive commands (`describe`, `--version`); human-readable text only inside the picker UI.
- Config: TOML at `%APPDATA%\atref\config.toml` on Windows. No environment-variable overrides in v0.1.
- Spec rules: `D:/jfuchs/dev/second-brain/Spec Writing Rules for Agents.md`. No "Future Considerations", no "Success Criteria"; every requirement traces to an Acceptance Criterion.
- Commits: do not commit unless JJ explicitly asks.

## Workflow

1. **READ** — Routing table → required files → cross-references.
2. **SEARCH** — Inspect existing specs and (when present) source.
3. **PLAN** — State the approach; flag scope creep against the spec's Out of Scope.
4. **IMPLEMENT** — Spec change first when behavior changes; code follows.
5. **VERIFY** — Run the spec's Acceptance Criteria against the code before responding.

## Current State

- Repository scaffolded with README, AGENTS.md, CLAUDE.md, LICENSE, CHANGELOG, and `specs/`.
- No source code yet.
- Spec 001 (`specs/001-windows-mvp.md`) is the next thing to implement. Status: pending.
- Strategic project node: `📦 atref.md` in JJ's second-brain vault.

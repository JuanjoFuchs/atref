---
id: "008"
title: atref packaging — crates.io, GitHub Release, and WinGet (Windows)
status: in_progress
blocked_by: []
blocks: []
---

# atref Packaging (Windows)

## Overview

Ship atref to the places Windows users install from: **crates.io** (`cargo install atref`),
a downloadable **`atref.exe`** on **GitHub Releases**, and the **Windows Package Manager
(WinGet)**. Publishing is automated from GitHub Actions on a version tag — never from a
developer machine — mirroring the proven ccburn / agent-mail pipeline, adapted for Rust.

atref is **Windows-only today** (it uses the `windows` crate + Win32 for caret/focus/region
and insertion), so it does not compile on macOS/Linux. This spec therefore ships a single
Windows x64 build. Homebrew, Linux/macOS binaries, and npm/`npx` distribution are **deferred
to after the cross-platform port (roadmap #22)** — a later npm spec will consume the GitHub
Release binary this spec produces, exactly as agent-mail's spec 003 consumed spec 002's.

The crate name `atref` is already reserved on crates.io (a `0.0.0` placeholder); this is the
first real publish. atref is a tray app that also exposes a CLI (`atref describe`,
`atref config …`, specs 006/007), so a portable install that puts `atref` on PATH is useful
for both humans (launch the tray) and agents (run the CLI).

> **Completion rule:** This spec is not complete until all acceptance criteria are verified,
> including real installs from real channels: `cargo install atref` on a clean Windows
> machine, the GitHub Release `atref.exe` runs, and `winget install JuanjoFuchs.atref` works
> on a clean Windows machine after Microsoft approval. Build-only and CI-only verification are
> insufficient. The agent must iterate until verification passes.

## As-built deviation — Tauri rewrite (2026-06-08)

After this spec was drafted, atref's GUI was rewritten from egui/eframe to
**Tauri 2 / WebView2** (commits `8726cdc`, `11a95a7`). That changed two things here:

- **crates.io is dropped for this round.** `cargo install atref` is not viable for
  a Tauri WebView app (the build embeds the static `ui/` frontend via `tauri-build`,
  which a source-only `cargo install` can't reliably reproduce). Published channels
  this round are **GitHub Release + WinGet only**. crates.io Trusted Publishing
  stays configured but unused; `cargo install` can be added later via a
  `[package] include` of the frontend assets, tested on a clean machine. (FR1/FR3 +
  the crates.io ACs are deferred; everything else stands.)
- **The artifact is the portable `atref.exe`** from `cargo build --release` —
  `tauri-build` (build.rs) embeds the frontend + icon, yielding a self-contained exe
  that runs both the tray app and the CLI (it uses the system WebView2, which ships
  with Windows 11). No NSIS/MSI installer and no `tauri-cli` this round; the WinGet
  **portable** install + `Commands: atref` puts the CLI on PATH.

Pipeline: `.github/workflows/{ci,release,winget-init,winget-publish}.yml`, adapted
from the agent-mail winget workflows.

**Channels added 2026-06-09:** beyond GitHub Release + WinGet, atref also ships via
**Scoop** (this repo doubles as a Scoop bucket — `bucket/atref.json`, auto-updated
by `.github/workflows/excavator.yml`; `scoop bucket add atref <repo>` then
`scoop install atref`) and a **PowerShell installer** (`install.ps1`, run as
`irm …/install.ps1 | iex` — downloads the latest `.exe` to a per-user dir + adds it
to PATH). Both serve the same portable `atref.exe`.

## Goals

- `cargo install atref`, a one-click `atref.exe` download, and `winget install
  JuanjoFuchs.atref` all install a working atref on a fresh Windows machine.
- Every tagged release builds and publishes its artifacts from GitHub Actions reproducibly.
- The release version is authored in exactly one place; a tag that disagrees fails the
  release before any artifact is created.
- Establish the GitHub Release binary that a future npm/`npx` spec (post-#22) will consume.

## Requirements

### Functional Requirements

- **FR1**: `cargo install atref` on Windows x64 builds and installs the `atref` binary; `atref
  describe` then runs. (Pure-Rust — no C toolchain required; the `.exe`-icon resource step
  degrades gracefully when a resource compiler is absent.)
- **FR2**: Each tagged release attaches a Windows x64 `atref.exe` to the GitHub Release, named
  so WinGet and a future npm wrapper can locate it predictably
  (`atref-<version>-windows-x64.exe`).
- **FR3**: `cargo publish` uploads the crate to crates.io for the tagged version.
- **FR4**: `winget install JuanjoFuchs.atref` installs atref on Windows after the manifest is
  approved, and `atref` is available on PATH (so `atref describe` / `atref config …` run from
  any directory).
- **FR5**: Installed/downloaded builds preserve all shipped behavior (specs 001–007): tray +
  chord + picker + insertion, and the CLI.
- **FR6**: A CI workflow runs the project gate (`cargo fmt --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test`) on every push/PR to `main`.

### Non-Functional Requirements

- **NFR1**: All published artifacts (crate, `.exe`, WinGet submission) are built/uploaded by
  GitHub Actions, not a developer machine.
- **NFR2**: Steady-state crates.io publishing uses Trusted Publishing (OIDC); a long-lived
  registry token is not used for steady-state publishing.
- **NFR3**: The release version is authored only in `Cargo.toml`; a tag-pushed release fails
  before artifact creation when the tag disagrees with that version.

### Technical Constraints

- **TC1**: Windows-only this round. The crates.io publish + binary build run on
  `windows-latest` (the crate does not compile elsewhere). This is intentionally a single-job
  release, not the examples' multi-runner matrix.
- **TC2**: crate name `atref` (crates.io, already reserved); WinGet identifier
  `JuanjoFuchs.atref`; release binary asset `atref-<version>-windows-x64.exe`.
- **TC3**: WinGet manifest is **portable**, includes `Commands: atref` and
  `UpgradeBehavior: uninstallPrevious`. Manifests live in `microsoft/winget-pkgs`, generated
  by `wingetcreate` — not committed to this repo.
- **TC4**: Version single-sourced from `Cargo.toml` `[package].version`; the release workflow
  validates the tag against it (NFR3).
- **TC5**: Implementation pattern is taken from ccburn / agent-mail (`D:/jfuchs/dev/ccburn`,
  `D:/jfuchs/dev/agent-mail-cli`): the implementer reads their `release.yml`,
  `winget-init.yml`, `winget-publish.yml`, and `ci.yml`, then substitutes the Rust build
  (`cargo build --release`, `cargo publish`) for the PyInstaller/twine steps. This spec
  defines the contracts; those repos define the proven YAML.

### Requirement Traceability

| Requirement | Acceptance Criteria |
|---|---|
| FR1 | AC5, AC8 |
| FR2 | AC3, AC4 |
| FR3 | AC6 |
| FR4 | AC9, AC10, AC11 |
| FR5 | AC7 |
| FR6 | AC1 |
| NFR1 | AC2, AC3, AC6, AC9 |
| NFR2 | AC6, AC12 |
| NFR3 | AC13 |

## Pre-requisites (Human Required)

GitHub-side setup may be done from the agent session with JJ's approval; registry/token steps
need the relevant web UI.

### crates.io
- [ ] crates.io account owns the existing `atref` crate (`0.0.0` placeholder already published).
- [ ] Configure **crates.io Trusted Publishing** for the `atref` crate → owner `JuanjoFuchs`,
      repo `atref`, workflow `release.yml`. (The crate already exists, so OIDC publishing works
      without a first-publish bootstrap.)
- [ ] *Fallback only if Trusted Publishing is unavailable:* create a scoped `CARGO_REGISTRY_TOKEN`
      repo secret for the first publish, then remove it once OIDC publishing succeeds (mirrors
      agent-mail's PyPI token→OIDC handoff).

### GitHub
- [ ] Create a `release` environment (Settings → Environments).

### WinGet
- [ ] Generate a GitHub Personal Access Token with `public_repo` scope; add it as the
      `WINGET_TOKEN` repo secret.
- [ ] After the first GitHub Release includes `atref.exe`, trigger the one-time WinGet
      submission workflow; verify the generated PR's installer manifest contains
      `UpgradeBehavior: uninstallPrevious` and `Commands: atref` before Microsoft review.

## Key Decisions

- **Single Windows release job, not a matrix.** Because atref only compiles on Windows today,
  one `windows-latest` job builds the `.exe`, runs `cargo publish`, uploads the release asset,
  and (via a separate workflow) submits to WinGet. The examples' cross-platform matrix returns
  with #22.
- **crates.io is a real channel even though it's source-build.** `cargo install atref` compiles
  on the user's machine; it stays C-toolchain-free (the reason redb was chosen over bundled
  SQLite), so it "just works" for Rust users on Windows. The `.exe`-icon resource step warns
  rather than fails if a resource compiler is missing, so `cargo install` never breaks over the
  icon (build.rs, spec #32).
- **WinGet portable with `Commands: atref`.** atref is a GUI tray app, but the portable install
  + `Commands` alias puts `atref` on PATH so the CLI (006/007) is usable; `UpgradeBehavior:
  uninstallPrevious` avoids duplicate entries on upgrade.
- **Version stays 0.5.x.** The crate version already encodes the shipped feature waves
  (v0.1–v0.5); the first public release is the current `Cargo.toml` version, not a reset to
  0.1.0. The CHANGELOG `[Unreleased]` section is promoted to the release version.
- **Reference the examples, don't reinvent the YAML.** ccburn/agent-mail are the working
  precedent; this spec adapts their workflows to Rust rather than inlining new ones.

## Implementation Tasks

- [ ] Promote `CHANGELOG.md` `[Unreleased]` to the release version; confirm `Cargo.toml`
      version is the intended public version.
- [ ] Add `.github/workflows/ci.yml` — run `cargo fmt --check`, `cargo clippy --all-targets --
      -D warnings`, `cargo test` on `windows-latest` for push/PR to `main`. (Headless tests
      only; the live-GUI e2e stays `#[ignore]`d.)
- [ ] Add `.github/workflows/release.yml` — on tag `v*`, on `windows-latest`: validate tag ==
      `Cargo.toml` version; `cargo build --release`; rename to `atref-<version>-windows-x64.exe`;
      smoke-run `atref --version`; `cargo publish` (OIDC Trusted Publishing); create the GitHub
      Release with the `.exe`; in the `release` environment.
- [ ] Add `.github/workflows/winget-init.yml` (manual, one-time) and
      `.github/workflows/winget-publish.yml` (on release published) using `wingetcreate` for
      `JuanjoFuchs.atref`, injecting `UpgradeBehavior: uninstallPrevious` + `Commands: atref`.
- [ ] Update `README.md` install section: `cargo install atref`, the GitHub Release download,
      and `winget install JuanjoFuchs.atref` (after approval). Mark npx / Homebrew / Linux /
      macOS as "after the cross-platform port (#22)".
- [ ] Update `📦 atref.md` roadmap #21 once shipped.

## Acceptance Criteria

### Agent-verifiable (local + CI + `gh`)
- [ ] **AC1** (`integration`): `ci.yml` exists, is valid YAML, and a CI run passes
      (`cargo fmt --check` + `clippy -D warnings` + `cargo test`). Verify via `gh run view`.
- [ ] **AC2** (`integration`): `release.yml`, `winget-init.yml`, `winget-publish.yml` exist and
      are valid YAML; no publishing step runs on a developer machine.
- [ ] **AC3** (`manual`/`integration`): after pushing `vX.Y.Z`, the GitHub Release for that tag
      has `atref-X.Y.Z-windows-x64.exe`. Verify via `gh release view vX.Y.Z`.
- [ ] **AC13** (`integration`): pushing a tag whose version differs from `Cargo.toml` fails the
      release workflow before any artifact is created.

### Real-install verification (human-in-the-loop)
- [ ] **AC4**: the released `atref-X.Y.Z-windows-x64.exe`, downloaded directly, launches the
      tray and `atref describe` runs.
- [ ] **AC5**: `cargo install atref` on a clean Windows x64 machine builds and installs `atref`;
      `atref describe` runs.
- [ ] **AC6**: crates.io shows `atref X.Y.Z` (`cargo search atref` / the crate page), published
      via OIDC Trusted Publishing (no token in the publish step after first release).
- [ ] **AC7**: an installed/downloaded build preserves shipped behavior — chord shows the
      picker, Enter inserts, and `atref config get` works (specs 001–007 smoke).
- [ ] **AC8**: `cargo install atref` succeeds even when a Windows resource compiler is absent
      (icon-embed warns, build proceeds).
- [ ] **AC9**: the initial WinGet workflow opens a PR to `microsoft/winget-pkgs`; the submitted
      installer manifest contains `UpgradeBehavior: uninstallPrevious` and `Commands: atref`.
- [ ] **AC10**: after Microsoft approval, `winget install JuanjoFuchs.atref` installs on a clean
      Windows x64 machine and `atref describe` runs from any directory.
- [ ] **AC11**: after a later release + WinGet publish, `winget upgrade JuanjoFuchs.atref` leaves
      exactly one entry (`winget list atref` → one row).
- [ ] **AC12**: after the first release, the crates.io token fallback (if used) is removed and a
      subsequent release publishes through OIDC with no registry token.

## Testing Approach

### Local validation (before pushing a tag)
```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
target/release/atref.exe --version
cargo publish --dry-run
```

### Release validation (after pushing `vX.Y.Z`)
```
gh run watch
gh release view vX.Y.Z      # has atref-X.Y.Z-windows-x64.exe
cargo search atref          # shows X.Y.Z
```

### Clean-machine verification
- `cargo install atref` on a fresh Windows machine → `atref describe`.
- Download the release `.exe` → it launches the tray + `atref describe` runs.
- After WinGet approval: `winget install JuanjoFuchs.atref` → `atref describe`; then after a
  later release, `winget upgrade JuanjoFuchs.atref` → `winget list atref` shows one row.

### Human-in-the-Loop Release Protocol
1. Agent: finish workflows + docs; pass local validation; `cargo publish --dry-run` clean.
2. Agent: ask JJ before pushing the release tag.
3. Human: approve the tag (and confirm crates.io Trusted Publishing in the web UI if the agent
   can't).
4. Agent: push the tag; monitor GitHub Actions; report crate + Release status.
5. Agent: trigger the WinGet initial submission; verify the PR includes the required manifest
   fields.
6. Human: confirm clean-machine `cargo install` / `.exe` run, and `winget install` after
   Microsoft approval, when local agent access is unavailable.

## Usage Examples

```powershell
winget install JuanjoFuchs.atref     # Windows Package Manager (after approval)
cargo install atref                  # build from source (Rust users)
# or download atref-<version>-windows-x64.exe from the GitHub Release and run it
atref                                 # launch the tray app
atref describe                        # CLI surface (for agents)
```

## Out of Scope

- **Homebrew, Linux, and macOS binaries** — atref doesn't compile off Windows yet; deferred to
  after the cross-platform port (roadmap #22).
- **npm / `npx` distribution** — a separate spec after #22, consuming this spec's GitHub Release
  binary (the npm name is also still blocked; scoped `@juanjofuchs/atref` is the fallback).
- **MSI / installer beyond WinGet portable**, **autostart-at-login** (roadmap #17),
  **code signing** (the `.exe` is unsigned, so SmartScreen may warn on first run — signing is a
  later decision).
- **A version reset to 0.1.0** — ship the current 0.5.x line.

## References
- ccburn packaging + npm: `D:/jfuchs/dev/ccburn/specs/002-packaging.md`,
  `D:/jfuchs/dev/ccburn/.github/workflows/` — the working PyPI/Release/WinGet precedent.
- agent-mail packaging + npm: `D:/jfuchs/dev/agent-mail-cli/specs/002-packaging.md` &
  `003-npm-distribution.md` — the token→OIDC handoff and scoped-name lessons.
- Project node + roadmap: `📦 atref.md` (capability #21 packaging; #22 cross-platform; the
  Distribution targets table) in JJ's vault.
- crates.io Trusted Publishing; WinGet `wingetcreate`:
  https://learn.microsoft.com/windows/package-manager/package/windows-package-manager-manifest-creator
- Testing seams: `ai-docs/testable-architecture.md`.

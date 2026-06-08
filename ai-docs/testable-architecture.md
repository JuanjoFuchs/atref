# Testable Architecture (atref)

**Purpose:** how atref is built so that an AI agent can validate almost every
acceptance criterion *by running code*, keeping human testing to an irreducible
minimum. **Read this before drafting or implementing any spec.**

## Why this exists

atref is built by AI agents iterating fast. A spec that can only be checked by
a human at the screen stalls iteration — the agent writes code, then waits. So
the architecture is shaped around one rule:

> **Every acceptance criterion should be verifiable by the agent through
> automated tests.** Manual testing is reserved for the few things that
> genuinely cannot be automated, and those are listed explicitly.

This is proven, not assumed — see the spike at
`D:\jfuchs\dev\atref-spike-testability` (egui_kittest input + image snapshot,
and a Win32 `EDIT` insertion round-trip — all green).

## The three seams

Design every feature so its behavior crosses one of three testable seams:

1. **Logic** — pure functions/structs, no egui, no OS. Config parsing,
   indexing/filtering rules, ranking, the matcher, the picker **state model**
   (query → results → selection → accepted item), the inserted-string format.
   → **unit tests.**
2. **The egui view** — the picker rendered by egui. → **`egui_kittest`**
   (headless): simulate input and assert the state model, and image-snapshot
   the rendering.
3. **The OS boundary** — clipboard, foreground window, global hotkey,
   file-watching. → **integration tests against a fixture the test owns and can
   read back** (a Win32 `EDIT` control, a temp dir, a temp git repo, synthetic
   events).

The architectural commitment that makes this work: **keep behavior out of the
render and OS layers.** The picker is a thin egui view over a state model;
insertion is split into compute-string / clipboard / paste; indexing is a pure
function fed by the walker. If logic gets tangled into `eframe::App::update` or
a Win32 call, it is not testable — refactor it back to a seam.

## The validation toolkit (proven in the spike)

### Unit tests — logic
Standard `cargo test`. Already used for `config` / `index` / `reference`.
Extend it to the picker state model so "type X → these results, selection Y,
accept → string Z" is a pure test.

### egui_kittest — the UI, headless
Dev-dependency: `egui_kittest` with features `wgpu` + `snapshot`. Two powers:

- **Input simulation + state assertions** — drives the *real* egui widget code:
  ```rust
  let mut h = Harness::new_ui_state(|ui, state| picker_ui(ui, state), state);
  h.run();
  h.input_mut().events.push(egui::Event::Text("agents".into())); // type
  h.run();
  h.key_press(egui::Key::ArrowDown);                              // navigate
  h.run();
  assert_eq!(h.state().selected, 1);                             // assert model
  ```
- **Image snapshots — the visual ACs:**
  ```rust
  h.snapshot("picker_empty"); // diffs against tests/snapshots/picker_empty.png
  ```
  First run: `UPDATE_SNAPSHOTS=1 cargo test` creates the baseline PNG (a
  one-time human glance to approve the look). Every run after is an automated
  pixel-diff — so monospace, "prettier", the selection highlight, the counter,
  the footer, and emoji legibility all become regression-tested. **Commit the
  baseline PNGs** to `tests/snapshots/`.

### Win32 fixture — insertion
The test creates a real `EDIT` control it owns, runs the save → set → paste →
restore logic, pastes via `WM_PASTE`, reads back via `WM_GETTEXT`, and asserts
the text plus clipboard restore. Deterministic (same-thread `SendMessage`, no
foreground/keystroke timing). Proven in the spike's `insertion_proof`.

### Integration — filesystem
`ignore`-crate git filtering over a temp git repo; `notify` watcher over a temp
dir (create/delete files, poll until the index converges). Headless, with a
timeout for determinism.

### Live GUI — the real `.exe`, via UIA
The one lane that needs the running app. `eframe` ships AccessKit, so the live
egui picker is a Windows UIA provider: launch the binary isolated (`ATREF_DIR`
seam), inject the global chord + typing with `enigo`, read the picker by element
name/role with `uiautomation`, and screenshot with `xcap`. `#[ignore]`d (it
takes desktop focus) and run deliberately. Full recipe + the two modes
(deterministic gate vs. ad-hoc "eyes") in `ai-docs/agentic-gui-testing.md`.

## Validation methods (use these labels in specs)

| Label | Means | Human? |
|---|---|---|
| `unit` | pure `cargo test` | no |
| `integration` | `cargo test` against a temp fixture (dir / git repo / `EDIT` control / synthetic event) | no |
| `kittest-input` | `egui_kittest` simulated input + state assertion | no |
| `kittest-snapshot` | `egui_kittest` image diff vs an approved baseline | one-time baseline approval, then no |
| `live-gui` | drive the running `.exe` with `enigo` + read it via Windows UIA + screenshot (`xcap`) | no (but takes desktop focus) |
| `manual` | a human looks or acts | yes — minimize |

## The irreducible manual sliver

The sliver shrank: the **global chord firing + the picker driving on the real
desktop** is now automated by the `live-gui` harness (UIA + `enigo` + `xcap`) —
see `ai-docs/agentic-gui-testing.md`. What remains genuinely manual is a single
spot-check:

- **Insertion into specific Electron apps** (Obsidian, VS Code) — the
  `EDIT`-control fixture proves the mechanism and `live-gui` proves the chord
  path; per-app paste quirks are a quick human eyeball.

Everything else should be automated.

## How to draft a spec against this

- Phrase each AC as a machine-checkable target ("counter reads `n/total`",
  "matches approved snapshot `picker_default.png`"), not a vibe.
- Tag every AC with a validation label from the table above. Prefer automated;
  if `manual`, justify why it is irreducible.
- Put new behavior behind a seam so it lands as `unit` / `integration` /
  `kittest`, not `manual`.

## How to implement against this

- Keep logic in the lib core (`config` / `index` / `reference` / picker-state);
  `main.rs` stays a thin wiring + view layer.
- Write the test as you implement the AC — do not defer to a human pass.
- Snapshot baselines live in `tests/snapshots/` and are committed.
- `cargo test` + `cargo clippy --all-targets -- -D warnings` +
  `cargo fmt --check` must pass before an AC is "met".

## References

- **Live-GUI testing:** `ai-docs/agentic-gui-testing.md` (driving the running
  app via UIA + `enigo` + `xcap`).
- Proof spike: `D:\jfuchs\dev\atref-spike-testability` (delete once specs 002/003
  carry these tests for real).
- Specs: `specs/*.md`.
- Spec rules: the vault's `Spec Writing Rules for Agents.md`.

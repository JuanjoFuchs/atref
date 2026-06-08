# Agentic GUI Testing (atref)

**Purpose:** how an AI agent iterates on atref's *live* GUI — launch the real
binary, drive it with OS input, read what it rendered, and screenshot it — all
without a human at the screen. This is the companion to
`ai-docs/testable-architecture.md`: that doc pushes behavior down to headless
seams; this one covers the thin top layer those seams can't reach — the running
`.exe`, the global chord, the on-screen picker. **Read this before testing or
iterating on anything that only manifests in the running app.**

## The one finding that makes this work

`eframe` enables **AccessKit by default** (`eframe = "0.33"`, no
`default-features = false`). AccessKit's Windows adapter subclasses the window
and answers `WM_GETOBJECT`, so **the running egui picker is a normal Windows UI
Automation (UIA) provider.** An external UIA client reads atref's widgets *by
name and role* — no pixel-hunting. Verified: the live picker's UIA subtree,
read by `uiautomation-rs` while the app ran, with the query `gamma` typed in:

```
Window "atref"
  Button "enter  insert"
  Button "esc  close"
  Button "↑↓  move"
  Text   "atref"
  Button "×"
  Text   "1 / 3"                      ← counter, readable as text
  Edit   ""                           ← query box (ValuePattern == "gamma")
  Button "gamma_widget.rs    files"   ← result row, named (filename + folder)
```

Consequence: assert the live app the same way the unit/kittest tests assert the
model — by *meaning*, not coordinates. Caveat: a widget only appears in the tree
while it is rendered (the picker's widgets exist only when it is visible), and
unlabelled widgets (e.g. a bare `TextEdit`) show an empty `Name` — read those
via `ValuePattern` instead.

## The toolset (all Rust, all in `cargo`)

| Tool | Role | Notes |
|---|---|---|
| `uiautomation` (leexgone) | **Observe + assert** the live UI by name/role | `UIAutomation::new()` inits COM; `create_matcher()…find_first/find_all`; `ValuePattern.get_value()`, `InvokePattern.invoke()` |
| `enigo` | **Inject** real OS input (global chord, typing) | already a runtime dep; what UIA can't do (fire a global hotkey) |
| `xcap` | **Screenshot** the composited desktop (incl. the GPU surface) | `Monitor::all()[0].capture_image().save(...)` |
| `ATREF_DIR` env seam | **Isolation** | overrides atref's config + `index.redb` dir so a run never touches the user's real `%APPDATA%\atref` |

Division of labour: **drive via OS input (`enigo`), observe via UIA
(`uiautomation`).** Keep those two jobs separate — it's why the harness is
stable.

## Where this sits in the testing stack

`ai-docs/testable-architecture.md` still owns the bottom three lanes. This adds
one on top that automates what that doc previously called the *manual* GUI
sliver.

| Layer | Tool | Covers |
|---|---|---|
| Logic | `unit` | ranking, config, reference formatting, picker state model |
| View | `egui_kittest` | picker state + pixel snapshots, **in-memory** |
| OS mechanisms | `integration` (Win32 `EDIT` fixture, temp dirs/git) | **insertion paste**, indexing, watcher |
| **Live GUI** | **`uiautomation` + `enigo` + `xcap`** (`live-gui`) | the **real `.exe`**: global chord fires, picker drives + reads on the actual desktop, screenshot |
| Manual (tiny) | spot-check | insertion into specific Electron apps |

**`live-gui` does not replace `egui_kittest`.** `egui_kittest` asserts egui's
*own internal* AccessKit tree in memory; it never proves the shipped binary
exposes that tree to an external UIA client, that the global hotkey triggers it,
or that it's screenshot-able. Keep using kittest for fast view tests; reach for
`live-gui` only for what genuinely needs the running OS app. And do **not**
re-prove insertion here — its mechanism belongs in the `integration` `EDIT`
fixture; a live insertion test would only re-cover that plus focus-timing.

## Two ways to use it

Both share `tests/common/mod.rs` (launch-isolated, find-window-by-pid, dump UIA,
fire chord, screenshot).

### The gate — deterministic `cargo test`
`tests/e2e.rs` — launch isolated → fire `Ctrl+Space` → type → assert the picker
via UIA (result Button present, counter text, query `ValuePattern`) → screenshot.
It is `#[ignore]`d so normal `cargo test` stays green and headless. Run it
deliberately:

```
cargo test --test e2e -- --ignored --nocapture
```

### The eyes — ad-hoc look
`examples/drive.rs` — launch atref, optionally drive it, then **screenshot +
dump the UIA tree** so the agent can *see* what it built (not a pass/fail test):

```
cargo build                          # ensure atref.exe exists
cargo run --example drive            # just launch + screenshot + dump
cargo run --example drive -- gamma   # also fire the chord and type "gamma"
```

Artifacts land in `target/e2e-artifacts/*.png`; read the PNG and the printed UIA
tree to reason about layout/behaviour.

## Rules of the road

- **These take keyboard focus** for a few seconds (global chord + synthetic
  typing). That's why the gate is `#[ignore]`d — run it when you (or the human)
  aren't typing elsewhere. Never put a `live-gui` test in the default `cargo
  test` path.
- **Isolation is mandatory.** Always launch via the `ATREF_DIR` seam against a
  temp config; never let a test point atref at the user's real folders or store.
- **Tag specs** with the `live-gui` label (alongside `unit` / `integration` /
  `kittest-*` / `manual`) when an AC can only be checked on the running app.
- **Local for now.** Today these run on the dev desktop. To remove even the
  focus-takeover nuisance, the same harness can later run inside a disposable
  **Windows Sandbox** or a **persistent VM** (it's just where the `.exe` and the
  input live) — deferred until the local loop proves itself.

## What's automated now vs. still manual

- **Now automated** (was manual): the global chord firing + the picker showing
  and being driven on the real desktop, plus screenshot evidence.
- **Still manual** (tiny spot-check): insertion into specific Electron apps
  (Obsidian, VS Code) — per-app paste quirks. The `EDIT`-fixture integration
  test proves the mechanism; this is just a human eyeballing the real apps.
- **De-scoped:** right-clicking the **tray icon** to exercise its menu. Not a
  priority. If ever needed it's automatable via a UIA walk of the
  "User Promoted Notification Area" (overflow needs the chevron opened first) —
  see the research note in JJ's vault.

## Decisions (resolved by the spike, 2026-06-08)

- **D1 — semantic UIA, not pixels.** AccessKit-in-eframe exposes named widgets +
  `ValuePattern`/counter text. Confirmed against the running binary.
- **D2 — run locally now**, graduate to Sandbox/VM later for desktop isolation.
- **D3 — Rust harness, no MCP.** A desktop MCP (Windows-MCP, etc.) was evaluated
  for the "eyes" but dropped: the Rust harness already screenshots and reads the
  live UIA tree, in-repo and dependency-light, without a standing server holding
  desktop-wide control. `examples/drive.rs` is the eyes instead.

## References

- Harness: `tests/common/mod.rs`, gate `tests/e2e.rs`, eyes `examples/drive.rs`.
- Seam: `ATREF_DIR` in `src/main.rs` (`fn main`).
- Sits under `ai-docs/testable-architecture.md` (the three headless seams).
- Research (tool landscape, sandboxes, tray automation): `📦 atref.md` +
  the agentic-GUI-testing research note in JJ's second-brain vault.

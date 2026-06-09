# atref

A global file-reference picker for Windows. Press a keyboard chord anywhere —
terminal, browser, Obsidian, chat app, IDE — and an unobtrusive fuzzy picker
pops up near the caret. Type to filter, hit **Enter**, and an `@"<absolute
path>"` reference to the selected file is inserted right where you were typing.

The pretty name is **@ref**. It runs as a system-tray app; it also exposes a
small CLI so agents can configure it.

> Windows-only today (it uses WebView2 + Win32 for the picker, caret, and
> insertion). macOS/Linux are a future port.

## Install

**WinGet** (after Microsoft approves the manifest):

```powershell
winget install JuanjoFuchs.atref
```

**Scoop:**

```powershell
scoop bucket add atref https://github.com/JuanjoFuchs/atref
scoop install atref
```

**PowerShell one-liner** (downloads the latest `.exe` to a per-user dir + adds it to PATH):

```powershell
irm https://raw.githubusercontent.com/JuanjoFuchs/atref/main/install.ps1 | iex
```

**Direct download:** grab `atref-<version>-windows-x64.exe` from the
[latest release](https://github.com/JuanjoFuchs/atref/releases/latest) and run it.

Every path installs the same portable executable (requires the Microsoft Edge
WebView2 runtime, which ships with Windows 11).

## Use

1. Launch `atref` — it lives in the system tray.
2. Press the chord (default **Ctrl+Space**) in any text field.
3. Type to fuzzy-filter your indexed files; recents/most-used lead an empty query.
4. **Enter** inserts `@"<absolute path>"` at the caret; **Esc** dismisses.

Right-click the tray icon to open the config, reload it, or quit.

## Configure

Config is JSON at `%APPDATA%\atref\config.json` (created on first run). Edits are
**hot-reloaded** — no restart, no manual reload:

```json
{
  "folders": ["C:\\Users\\you\\dev", "C:\\Users\\you\\vault"],
  "exclude": [".git", "node_modules", "target"],
  "chord": "Control+Space",
  "git_aware": true
}
```

- `folders` — directories to index (recursively).
- `exclude` — directory names pruned during traversal.
- `chord` — the global summon hotkey (`global-hotkey` syntax).
- `git_aware` — follow `.gitignore` in Git repos (skip ignored files; still show
  untracked, non-ignored ones).

### Configure from the CLI (for agents)

The same binary is a small config CLI — discover the surface with `atref
describe` (JSON), then mutate `config.json` (validated, atomic; the running app
hot-reloads the change):

```powershell
atref describe                          # JSON schema of the commands + config
atref add                               # add the current directory to folders
atref config add folders D:\proj        # add a folder
atref config set chord "Control+Alt+Space"
atref config get                        # print the current config as JSON
```

## Features

- Fuzzy path matching (`nucleo-matcher`, basename-weighted, smart-case, CamelHumps).
- **Frecency** — recents/most-used surface first on an empty query and break
  near-equal matches.
- Persistent on-disk index (redb) — instant launch, reconciled in the background.
- Live file-watcher and git-aware indexing.
- Raycast-style picker (transparent borderless WebView2 window with acrylic).

## Build from source

```powershell
cargo build --release          # produces target\release\atref.exe
cargo test                     # headless tests (live-GUI e2e are #[ignore]d)
```

## License

MIT — see [LICENSE](LICENSE).

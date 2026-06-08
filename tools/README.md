# tools/ — icon-design utilities

Dev-time helpers for authoring atref's icon (`assets/icon.svg`). They are **not**
part of the shipped binary — they exist so an AI agent (or a human) can *see and
measure* an SVG while iterating on it, with no image library, Node, or MCP server.
Everything is driven by the system Chromium browser (Edge, then Chrome) in headless
mode. **Read `ai-docs/icon-design.md` for the technique and the "why".**

| Script | What it answers | Borrowed from |
|---|---|---|
| `render-svg.ps1` | "What does it look like?" — SVG → PNG, any zoom | render-in-the-loop |
| `icon-debug.ps1` | "Is it centered / well-margined?" — grid + center crosshair + equal-margin box at 2x | SVG-MCP *coordinate-mapping + zoom*, made visual |
| `measure-glyph.ps1` | "How big is the glyph's ink box, and what transform centers it?" | SVG-MCP *coordinate-mapping*, made quantitative |

`_common.ps1` holds the shared browser-detection and headless render/dump-dom helpers;
the three scripts dot-source it.

## Quick start (PowerShell)

```powershell
# Preview the icon at 2x and open it
./tools/render-svg.ps1 -Scale 2 -Open

# Verify centering — overlay grid + crosshair + equal-margin box
./tools/icon-debug.ps1 -Open

# (Glyph-based variant) survey which fonts have a near-square @, then get a centering matrix
./tools/measure-glyph.ps1 -Survey
./tools/measure-glyph.ps1 -Char '@' -Font "'Segoe UI'" -Target 184
```

Each script has full comment-based help: `Get-Help ./tools/render-svg.ps1 -Detailed`.

## Notes

- **Windows-first**, matching atref today. The approach is portable (any Chromium
  honours `--headless --screenshot`); only the browser paths in `_common.ps1` are
  Windows-specific.
- Outputs go to `$env:TEMP` by default, so nothing lands in the repo.
- These cover preview + measurement only. Building the multi-resolution `.ico`
  (roadmap #32) is a separate step — see `ai-docs/icon-design.md`.

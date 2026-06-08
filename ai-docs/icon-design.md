# Icon Design (atref)

**Purpose:** how to author and iterate on atref's icon (`assets/icon.svg`) as an AI
agent — render-in-the-loop, measure instead of eyeball, and hand-author for control.
**Read this before touching the icon or building the `.ico` (roadmap #32).**

## Why this exists

An agent can't design what it can't see, and "looks centered" is a guess. Early
icon work stalled on exactly that: nudging the `@` by feel, re-reading the SVG, and
hoping. The fix is the same discipline as `testable-architecture.md` — turn a visual
judgement into a *measured* one the agent can make itself:

> **Every visual claim ("centered", "even margins", "the right size glyph") should be
> verifiable by rendering and measuring, not by eyeballing the source.**

The mechanism is three small tools in `tools/` that drive a headless Chromium browser
(Edge/Chrome) — no image library, no Node, no MCP server. The techniques were borrowed
from SVG-rendering MCP servers (see *Prior art* below) and folded into the loop
*without* taking on the dependency.

## Three principles

1. **Hand-author, don't generate.** Prompt-to-vector tools (SVG AI, SVGMaker) give up
   precise control. The exact geometric `@` came from hand-written SVG paths, where
   every coordinate is intentional. Generators are for ideation, not the final mark.
2. **Render in the loop.** After every edit, render the SVG to PNG and *look*
   (`render-svg.ps1`). At actual size for the silhouette; at 2–4× to inspect stroke
   ends, joins, and corner radii.
3. **Measure, don't eyeball.** Overlay a coordinate grid + a dead-center crosshair +
   an equal-margin box (`icon-debug.ps1`), or read a glyph's true ink box and compute
   its centering transform (`measure-glyph.ps1`). This is the SVG-MCP "coordinate
   mapping + zoom" idea, made local.

## The toolkit (`tools/`)

| Tool | Answers | How |
|---|---|---|
| `render-svg.ps1` | "What does it look like?" | headless `--screenshot`, any `--force-device-scale-factor` |
| `icon-debug.ps1` | "Is it centered / evenly margined?" | grid + center crosshair + dashed equal-margin box, rendered at 2× |
| `measure-glyph.ps1` | "How big is the glyph, and what transform centers it?" | off-screen `<text>` → `getBBox()` via `--dump-dom` → `matrix()` |

`_common.ps1` provides `Find-Browser`, `Invoke-HeadlessShot`, `Invoke-HeadlessDumpDom`.
Full usage: `Get-Help ./tools/<script>.ps1 -Detailed`, or `tools/README.md`.

## Icon craft (the knowledge)

- **Container = a squircle tile.** A rounded square (`rect ... rx`) with a vertical
  brand-teal gradient (`#1FB89C → #12907A`). Rounded corners read as "app icon" across
  Windows tray, taskbar, and store contexts.
- **Optical centering beats geometric centering.** A shape centered by its bounding box
  often *looks* off. The crosshair overlay caught the inner "a" of the `@` sitting
  ~25px low; nudging it until the crosshair ran dead-center through the bowl fixed it.
  Trust the overlay, not the math.
- **Equal margins (keyline).** The dashed reference box in `icon-debug.ps1` (inset ~48px
  on a 256 canvas) checks the mark breathes evenly inside the tile — no edge crowding.
- **Stroke-based marks: round caps + joins.** The `@` is `stroke` (not `fill`),
  `stroke-width: 28`, `stroke-linecap/linejoin: round`. Round ends give a friendly,
  modern feel and survive downscaling better than sharp corners.
- **Snap to a grid.** Author coordinates on the overlay's 32px grid where possible —
  alignment you can see is alignment you can defend.
- **Design for the smallest size.** The icon must read at 16×16 (tray). Render at 1×
  *and* check that the silhouette survives — thin strokes and tight gaps vanish small.

## Two routes we tried (and why geometric won)

- **Glyph-based `@`** — set an actual `@` text glyph in a font, then center its ink box
  with a computed `matrix()` from `measure-glyph.ps1`. Clean idea, but a survey of the
  `@` across Windows fonts (`measure-glyph.ps1 -Survey`) showed **none are square** —
  Consolas 0.47, Segoe UI 0.72, the roundest (Arial) only 0.90 aspect. A non-square `@`
  forces either distortion or off-center mass. Abandoned.
- **Geometric `@`** (shipped) — three hand-drawn stroked paths: an outer ring (rounded
  square, open at lower-right), an inner "a" bowl, and an "a" stem + tail that connects
  into the ring's opening. Full control over weight, squareness, and the tail join.
  This is what's in `assets/icon.svg`.

`measure-glyph.ps1` is kept regardless — the ink-box-centering technique is the right
tool any time a future mark *is* glyph-based.

## Building the `.ico` (roadmap #32 — still open)

Preview/measurement is covered; producing the multi-resolution Windows icon is not.
An `.ico` should embed several sizes (16/24/32/48/256) so Windows picks the crisp one
per context. Options, cheapest first:

- A favicon/SVG→ICO converter (e.g. a "Favicon MCP" if ever installed, or ImageMagick
  `magick icon.svg -define icon:auto-resize=256,48,32,24,16 icon.ico`).
- Render each size with `render-svg.ps1 -Size N` and pack them into an `.ico`.

Then embed it as the `.exe`/taskbar icon and reuse it for the tray (the #32 deliverable).

## Prior art — SVG MCP servers (surveyed, not installed)

These exist and do this well; we borrowed their *techniques* rather than the dependency:

- **SVG-MCP** (adamryczkowski) — renders SVG→PNG **and returns coordinate mapping** for
  iterative refinement. The direct inspiration for `icon-debug.ps1` and `measure-glyph.ps1`.
- **mcp-svg-converter** (surferdot) / **svg-maker-mcp** (erkamkavak) — SVG→PNG
  preview/validate/optimize; the inspiration for `render-svg.ps1`.
- **Favicon MCP** — SVG→ICO+PNG; the candidate for the #32 `.ico` build.

If the manual loop ever feels heavy, installing SVG-MCP would make rendering +
coordinate feedback first-class. It changes the *mechanism*, not the design principles
above.

## How to iterate on the icon

1. Edit `assets/icon.svg` (hand-author paths/coords).
2. `./tools/render-svg.ps1 -Scale 2 -Open` — look at the result.
3. `./tools/icon-debug.ps1 -Open` — confirm centering + even margins against the grid.
4. Adjust coordinates by the amount the overlay shows; repeat 2–3 until it sits right.
5. Check it at 1× and mentally at 16×16 (does the silhouette survive?).
6. For a glyph-based mark, use `measure-glyph.ps1` to get the centering `matrix()`.

## References

- Tools: `tools/` (+ `tools/README.md`).
- Current icon: `assets/icon.svg`.
- Companion doc / same discipline applied to code: `ai-docs/testable-architecture.md`.
- Roadmap context: `📦 atref.md` in JJ's vault (capability #32, designed app + tray icon).

<#
.SYNOPSIS
  Render an SVG (or any local HTML) to PNG via headless Edge/Chrome — the quick
  "see what I just drew" step of the icon-design loop.

.DESCRIPTION
  No image library or MCP needed: it drives the system Chromium browser headlessly.
  Use -Scale 2 (or higher) to zoom in for pixel/edge inspection. The PNG is written
  to TEMP by default. See ai-docs/icon-design.md.

.PARAMETER Path
  The SVG/HTML to render. Defaults to the repo's assets/icon.svg.

.PARAMETER Out
  Output PNG path. Defaults to $env:TEMP\atref-render.png.

.PARAMETER Scale
  Device scale factor (zoom). 1 = actual size, 2 = 2x, 4 = 4x.

.PARAMETER Size
  Square window size in CSS px (the SVG viewport). Default 256.

.PARAMETER Open
  Open the rendered PNG in the default viewer when done.

.EXAMPLE
  ./tools/render-svg.ps1 -Open
  Render assets/icon.svg at actual size and open it.

.EXAMPLE
  ./tools/render-svg.ps1 -Path assets/icon.svg -Scale 4 -Open
  4x zoom to inspect stroke ends and corner radii.
#>
[CmdletBinding()]
param(
    [string]$Path = (Join-Path $PSScriptRoot '..\assets\icon.svg'),
    [string]$Out = (Join-Path $env:TEMP 'atref-render.png'),
    [int]$Scale = 1,
    [int]$Size = 256,
    [switch]$Open
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '_common.ps1')

$uri = ConvertTo-FileUri $Path
$png = Invoke-HeadlessShot -Uri $uri -Out $Out -Scale $Scale -Size $Size
Write-Host "Rendered $Path -> $png  (${Size}px @ ${Scale}x)"
if ($Open) { Invoke-Item $png }

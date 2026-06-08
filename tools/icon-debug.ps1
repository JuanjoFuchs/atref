<#
.SYNOPSIS
  Render the icon with a measurement overlay — a coordinate grid, a dead-center
  crosshair, and an equal-margin reference box — so "is it centered?" becomes a
  measurement instead of a guess.

.DESCRIPTION
  This is the durable technique borrowed from the SVG-MCP server's idea of
  "coordinate mapping + zoom rendering": layer a pixel grid and a center crosshair
  over the rendered artwork at 2x, plus a dashed square showing equal margins from
  every edge. If the artwork's *optical* center doesn't sit on the crosshair you can
  read off exactly how far to nudge it. (This is how the inner "a" of the @ was
  caught sitting ~25px low and corrected.) See ai-docs/icon-design.md.

.PARAMETER Path
  SVG to inspect. Defaults to assets/icon.svg.

.PARAMETER Out
  Output PNG. Defaults to $env:TEMP\atref-icon-debug.png.

.PARAMETER Canvas
  Icon viewport size in px — must match the SVG's width/height. Default 256.

.PARAMETER Grid
  Grid spacing in px. Default 32.

.PARAMETER Margin
  Inset (px) of the equal-margin reference box from each edge. Default 48.

.PARAMETER Scale
  Device scale factor for the render. Default 2 (zoomed for detail).

.PARAMETER Open
  Open the rendered PNG when done.

.EXAMPLE
  ./tools/icon-debug.ps1 -Open
  Overlay grid + crosshair + margin box on assets/icon.svg at 2x and open it.
#>
[CmdletBinding()]
param(
    [string]$Path = (Join-Path $PSScriptRoot '..\assets\icon.svg'),
    [string]$Out = (Join-Path $env:TEMP 'atref-icon-debug.png'),
    [int]$Canvas = 256,
    [int]$Grid = 32,
    [int]$Margin = 48,
    [int]$Scale = 2,
    [switch]$Open
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '_common.ps1')

$svgUri = ConvertTo-FileUri $Path
$center = $Canvas / 2
$boxSize = $Canvas - 2 * $Margin

# Overlay: faint grid every $Grid px, a bright crosshair through the center, and a
# dashed square inset $Margin px on all sides (equal-margin keyline reference).
$html = @"
<!doctype html><html><head><meta charset="utf-8"></head>
<body style="margin:0">
<div style="position:relative;width:${Canvas}px;height:${Canvas}px">
  <img src="$svgUri" width="$Canvas" height="$Canvas" style="position:absolute;left:0;top:0"/>
  <svg id="ov" width="$Canvas" height="$Canvas" style="position:absolute;left:0;top:0"></svg>
</div>
<script>
  var ns='http://www.w3.org/2000/svg', ov=document.getElementById('ov');
  function ln(x1,y1,x2,y2,c,w){var l=document.createElementNS(ns,'line');
    l.setAttribute('x1',x1);l.setAttribute('y1',y1);l.setAttribute('x2',x2);l.setAttribute('y2',y2);
    l.setAttribute('stroke',c);l.setAttribute('stroke-width',w);ov.appendChild(l);}
  for(var i=$Grid;i<$Canvas;i+=$Grid){ ln(i,0,i,$Canvas,'rgba(255,80,80,.30)',1); ln(0,i,$Canvas,i,'rgba(255,80,80,.30)',1); }
  ln($center,0,$center,$Canvas,'red',1.4); ln(0,$center,$Canvas,$center,'red',1.4);     // center crosshair
  var r=document.createElementNS(ns,'rect');                                            // equal-margin box
  r.setAttribute('x',$Margin); r.setAttribute('y',$Margin);
  r.setAttribute('width',$boxSize); r.setAttribute('height',$boxSize);
  r.setAttribute('fill','none'); r.setAttribute('stroke','cyan'); r.setAttribute('stroke-width',1.4); r.setAttribute('stroke-dasharray','5 4');
  ov.appendChild(r);
</script>
</body></html>
"@

$tmp = Join-Path $env:TEMP 'atref-icon-debug.html'
Set-Content -LiteralPath $tmp -Value $html -Encoding utf8
$uri = ConvertTo-FileUri $tmp
$png = Invoke-HeadlessShot -Uri $uri -Out $Out -Scale $Scale -Size $Canvas
Write-Host "Debug overlay (grid ${Grid}px, crosshair @ $center, margin box inset ${Margin}px) -> $png"
if ($Open) { Invoke-Item $png }

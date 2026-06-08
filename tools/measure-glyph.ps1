<#
.SYNOPSIS
  Measure a text glyph's rendered ink box and emit the SVG transform that centers it
  — the quantitative "coordinate mapping" technique (no eyeballing).

.DESCRIPTION
  Renders an off-screen <text> glyph headlessly, reads its getBBox() via --dump-dom,
  then prints a ready-to-paste matrix(sx,0,0,sy,tx,ty) that scales the ink box to
  -Target px and centers it on the -Canvas. With -Survey it instead lists the @
  aspect ratio across candidate Windows fonts (this is how a near-square @ was chosen
  for the glyph-based icon variant before the geometric @ was hand-drawn instead).
  See ai-docs/icon-design.md.

  Glyph centering matters because a font's ink box is offset from its layout origin
  (the baseline sits well below the visual middle), so a naive x=0 y=0 places the
  glyph wildly off. Measuring the real ink box removes the guesswork.

.PARAMETER Char
  The glyph to measure/center. Default '@'.

.PARAMETER Font
  CSS font-family (quote multi-word names). Default "'Segoe UI'".

.PARAMETER Target
  Target ink-box size in px — the glyph is scaled to fill this square. Default 184.

.PARAMETER Canvas
  Icon viewport size in px. Default 256.

.PARAMETER Stretch
  Scale width and height independently so the ink box exactly fills a Target square
  (distorts aspect). Default is uniform scale (preserves the glyph's shape).

.PARAMETER Survey
  List @ bbox width/height/aspect across candidate fonts instead of computing a matrix.

.EXAMPLE
  ./tools/measure-glyph.ps1 -Survey
  Compare how square the @ is in each candidate font.

.EXAMPLE
  ./tools/measure-glyph.ps1 -Char '@' -Font "'Segoe UI'" -Target 184
  Print the <text transform="..."> matrix that centers a 184px @ on a 256px canvas.
#>
[CmdletBinding()]
param(
    [string]$Char = '@',
    [string]$Font = "'Segoe UI'",
    [int]$Target = 184,
    [int]$Canvas = 256,
    [switch]$Stretch,
    [switch]$Survey
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '_common.ps1')

if ($Survey) {
    $html = @"
<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:#fff">
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text id="t" font-size="500">$Char</text></svg>
<pre id="out">PENDING</pre>
<script>
  var fonts = [
    ["Consolas","Consolas"], ["Cascadia Code","'Cascadia Code'"], ["Cascadia Mono","'Cascadia Mono'"],
    ["Courier New","'Courier New'"], ["Lucida Console","'Lucida Console'"], ["Segoe UI","'Segoe UI'"],
    ["Arial","Arial"], ["Verdana","Verdana"], ["Tahoma","Tahoma"], ["monospace","monospace"], ["sans-serif","sans-serif"]
  ];
  var t = document.getElementById('t'), lines = [];
  for (var i=0;i<fonts.length;i++){
    t.setAttribute('font-family', fonts[i][1]); t.textContent = "$Char";
    var b = t.getBBox();
    lines.push(fonts[i][0] + ' | w=' + b.width.toFixed(1) + ' h=' + b.height.toFixed(1) + ' aspect=' + (b.width/b.height).toFixed(3));
  }
  document.getElementById('out').textContent = 'SURVEY\n' + lines.join('\n');
</script>
</body></html>
"@
    $tmp = Join-Path $env:TEMP 'atref-glyph-survey.html'
    Set-Content -LiteralPath $tmp -Value $html -Encoding utf8
    $dom = Invoke-HeadlessDumpDom (ConvertTo-FileUri $tmp)
    if ($dom -match '(?s)SURVEY\s*(.+?)</pre>') { Write-Host ("@ aspect by font (closest to 1.000 is most square):`n" + $matches[1].Trim()) }
    else { Write-Host "No survey output captured."; Write-Host $dom }
    return
}

# --- measure mode: get the chosen glyph's ink box, compute a centering matrix ---
$html = @"
<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:#fff">
<svg xmlns="http://www.w3.org/2000/svg" width="$Canvas" height="$Canvas"><text id="t" font-size="500">$Char</text></svg>
<pre id="out">PENDING</pre>
<script>
  var t = document.getElementById('t');
  t.setAttribute('font-family', "$Font"); t.textContent = "$Char";
  var b = t.getBBox();
  document.getElementById('out').textContent =
    'BBOX ' + b.x.toFixed(3) + ' ' + b.y.toFixed(3) + ' ' + b.width.toFixed(3) + ' ' + b.height.toFixed(3);
</script>
</body></html>
"@
$tmp = Join-Path $env:TEMP 'atref-glyph-measure.html'
Set-Content -LiteralPath $tmp -Value $html -Encoding utf8
$dom = Invoke-HeadlessDumpDom (ConvertTo-FileUri $tmp)

if ($dom -match 'BBOX (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+)') {
    $bx = [double]$matches[1]; $by = [double]$matches[2]
    $bw = [double]$matches[3]; $bh = [double]$matches[4]
    $c = $Canvas / 2.0
    if ($Stretch) { $sx = $Target / $bw; $sy = $Target / $bh }
    else { $s = $Target / [math]::Max($bw, $bh); $sx = $s; $sy = $s }
    $tx = $c - $sx * ($bx + $bw / 2)
    $ty = $c - $sy * ($by + $bh / 2)
    $matrix = "matrix({0},0,0,{1},{2},{3})" -f `
        [math]::Round($sx, 5), [math]::Round($sy, 5), [math]::Round($tx, 3), [math]::Round($ty, 3)
    Write-Host ("glyph '{0}' in {1}: ink box x={2} y={3} w={4} h={5} (aspect {6})" -f `
            $Char, $Font, $bx, $by, $bw, $bh, [math]::Round($bw / $bh, 3))
    Write-Host ("scale mode: " + ($(if ($Stretch) { "stretch (non-uniform)" } else { "uniform" })))
    Write-Host "`nPaste into the SVG <text> element:"
    Write-Host ('  <text x="0" y="0" font-family={0} font-size="500" transform="{1}">{2}</text>' -f $Font, $matrix, $Char)
}
else {
    Write-Host "No BBOX captured — is the font installed? Raw DOM:"; Write-Host $dom
}

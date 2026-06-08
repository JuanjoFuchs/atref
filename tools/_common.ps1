# tools/_common.ps1 — shared helpers for the icon-design tooling.
#
# Dot-source from a sibling script:  . (Join-Path $PSScriptRoot '_common.ps1')
#
# No image library, no Node, no MCP: these drive the system Chromium browser
# (Edge preferred, then Chrome) in headless mode to render SVG/HTML to PNG and to
# read the post-script DOM. See ai-docs/icon-design.md for the "why".

function Find-Browser {
    # Locate a Chromium-family browser. Edge ships with Windows, so it is tried first.
    $candidates = @(
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe"
    )
    $found = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $found) {
        $found = (Get-Command msedge, chrome -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    }
    if (-not $found) {
        throw "No Chromium browser (Edge/Chrome) found — needed to render SVG headlessly."
    }
    return $found
}

function ConvertTo-FileUri {
    param([Parameter(Mandatory)][string]$Path)
    $abs = (Resolve-Path -LiteralPath $Path).Path
    return "file:///" + ($abs -replace '\\', '/')
}

function Invoke-HeadlessShot {
    # Screenshot a local SVG/HTML file to PNG via headless Chromium.
    #   -Scale = device scale factor (1 = actual size, 2 = 2x zoom for detail).
    #   -Size  = square window size in CSS px (should match the SVG viewport).
    param(
        [Parameter(Mandatory)][string]$Uri,
        [Parameter(Mandatory)][string]$Out,
        [int]$Scale = 1,
        [int]$Size = 256
    )
    if (Test-Path $Out) { Remove-Item $Out -Force }
    $browser = Find-Browser
    # A private user-data-dir avoids "browser already running" clashes with the user's daily Edge.
    $udd = Join-Path $env:TEMP 'atref-headless-profile'
    & $browser --headless=new --disable-gpu --no-sandbox --hide-scrollbars `
        --allow-file-access-from-files --user-data-dir="$udd" `
        --default-background-color=00000000 `
        --force-device-scale-factor=$Scale `
        --window-size="$Size,$Size" `
        --screenshot="$Out" $Uri 2>$null | Out-Null
    if (-not (Test-Path $Out)) { throw "Render failed: no PNG written to $Out" }
    return $Out
}

function Invoke-HeadlessDumpDom {
    # Render a local HTML file headlessly and return the post-script DOM as text.
    # Used to read measurements a page computes in JS (e.g. SVG getBBox()).
    param([Parameter(Mandatory)][string]$Uri)
    $browser = Find-Browser
    $udd = Join-Path $env:TEMP 'atref-headless-profile'
    return (& $browser --headless=new --disable-gpu --no-sandbox `
            --allow-file-access-from-files --user-data-dir="$udd" `
            --dump-dom $Uri 2>$null | Out-String)
}

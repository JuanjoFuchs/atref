#Requires -Version 5
<#
.SYNOPSIS
  Install atref: download the latest atref.exe from GitHub Releases into a
  per-user directory and add it to your PATH. No admin required.

.EXAMPLE
  irm https://raw.githubusercontent.com/JuanjoFuchs/atref/main/install.ps1 | iex
#>
$ErrorActionPreference = 'Stop'
$repo = 'JuanjoFuchs/atref'
$dir = Join-Path $env:LOCALAPPDATA 'Programs\atref'

Write-Host 'Installing atref...'
New-Item -ItemType Directory -Force $dir | Out-Null

$rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{
    'User-Agent' = 'atref-install'
    'Accept'     = 'application/vnd.github+json'
}
$asset = $rel.assets | Where-Object { $_.name -match 'windows-x64\.exe$' } | Select-Object -First 1
if (-not $asset) { throw "No windows-x64 executable in the latest release ($($rel.tag_name))." }

$exe = Join-Path $dir 'atref.exe'
Write-Host "Downloading $($asset.name) ($($rel.tag_name))..."
Invoke-WebRequest $asset.browser_download_url -OutFile $exe

# Add the install dir to the user PATH if it isn't already there (idempotent).
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $dir) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ';' + $dir), 'User')
    $env:Path += ";$dir"
    Write-Host "Added $dir to your PATH."
}

Write-Host ''
Write-Host "atref $($rel.tag_name) installed to $exe" -ForegroundColor Green
Write-Host "  Run 'atref' to start the tray app (Ctrl+Space summons the picker)."
Write-Host "  Run 'atref describe' for the agent CLI."
Write-Host '  Open a new terminal for the PATH change to take effect.'

# Takes the README screenshots from demo data. Nothing here reads real activity:
# the demo flag fills a throw-away data folder, the tracker is never started, and the
# window saves its own frames (no screen capture, no input events).
#
#   powershell -File tools\screenshots.ps1 -Exe target\release\Timelyx.exe -Out assets\screenshots
param(
    [string]$Exe = "target\release\Timelyx.exe",
    [string]$Out = "assets\screenshots",
    [string]$Work = (Join-Path $env:TEMP "timelyx-shots")
)
$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$runs = @(
    @{ theme = "dark";  lang = "en"; navs = "today,trends,apps,settings,categories,limits" },
    @{ theme = "light"; lang = "en"; navs = "today,trends,apps,settings,categories,limits" },
    @{ theme = "dark";  lang = "ru"; navs = "today" }
)
foreach ($r in $runs) {
    $data = Join-Path $Work "data-$($r.theme)-$($r.lang)"
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
    $env:TIMELYX_DATA_DIR = $data
    $env:TIMELYX_DEMO = "1"
    $env:TIMELYX_NO_TRACKER = "1"
    $env:TIMELYX_THEME = $r.theme
    $env:TIMELYX_LANG = $r.lang
    $env:TIMELYX_SHOT_DIR = (Resolve-Path $Out).Path
    $env:TIMELYX_SHOT_PREFIX = "$($r.theme)-$($r.lang)"
    $env:TIMELYX_SHOT_NAVS = $r.navs
    $p = Start-Process -FilePath $Exe -PassThru
    if (-not $p.WaitForExit(120000)) { $p.Kill(); throw "timed out" }
}
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue

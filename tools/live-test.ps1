# Live test of the Windows tracker. It never records real activity:
#  - the tracker runs from a temp copy with its own data folder (TIMELYX_DATA_DIR);
#  - TIMELYX_TRACK_ONLY makes it ignore every window except the test windows below;
#  - the test windows are copies of powershell.exe renamed chrome.exe and Code.exe;
#  - the only thing sent anywhere is a "bring this window forward" call to OUR OWN windows, and
#    the script checks that the foreground window really is ours before it goes on (otherwise it stops);
#  - autostart is never installed; only our own processes are killed.
#
#   powershell -File tools\live-test.ps1                      # the scenario, then a check of the database
#   powershell -File tools\live-test.ps1 -MeasureMinutes 10   # CPU and memory of the tracker with no window open
param(
    [string]$Exe = "target\release\Timelyx.exe",
    [int]$MeasureMinutes = 0,
    [string]$Report = ""
)
$ErrorActionPreference = "Stop"
$Exe = (Resolve-Path $Exe).Path
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Join-Path $env:TEMP "timelyx-live-$PID"
$apps = Join-Path $root "apps"
$data = Join-Path $root "data"
New-Item -ItemType Directory -Force $apps, $data | Out-Null
Copy-Item $Exe (Join-Path $root "Timelyx.exe")
$ps = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
Copy-Item $ps (Join-Path $apps "chrome.exe")
Copy-Item $ps (Join-Path $apps "Code.exe")
# Idle detection is switched off here: nobody types into the test windows, so every window would look idle.
# (The idle rules are covered by the unit tests.)
'{"onboarded": true, "language": "en", "idle_minutes": 0, "track_sites": true, "check_updates": false}' | Set-Content -Encoding ascii (Join-Path $data "config.json")

$env:TIMELYX_DATA_DIR = $data
$env:TIMELYX_TRACK_ONLY = $apps
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern void SwitchToThisWindow(IntPtr h, bool alt);
}
"@
function Foreground-Pid { $h = [W]::GetForegroundWindow(); [uint32]$p = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$p); return [int]$p }

$mine = New-Object System.Collections.ArrayList
function Stop-Mine {
    foreach ($id in $mine) { try { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue } catch {} }
}
function Start-Mine($file, $argList) {
    $p = Start-Process -FilePath $file -ArgumentList $argList -PassThru -WindowStyle Hidden
    [void]$mine.Add($p.Id)
    return $p
}
$handleN = 0
function Open-Window($exeName, $titles, $at) {
    # Started through Explorer: while the desktop or Explorer is in front, Windows lets a program that Explorer
    # starts take the foreground, which a background script cannot do by itself.
    $script:handleN++
    $hf = Join-Path $root "hwnd-$($script:handleN).txt"
    $script = Join-Path $here "live-window.ps1"
    $titleArgs = '"' + ($titles -join '|') + '"'
    $cmd = Join-Path $root "go-$($script:handleN).cmd"
    $line = "start `"`" `"$(Join-Path $apps $exeName)`" -NoProfile -ExecutionPolicy Bypass -File `"$script`" -TitleList $titleArgs -At $at -Lifetime 900 -HandleFile `"$hf`""
    "@echo off`r`n$line" | Set-Content -Encoding ascii $cmd
    Start-Process -FilePath explorer.exe -ArgumentList "`"$cmd`""
    return [pscustomobject]@{ HandleFile = $hf; Pid = 0 }
}function Read-Hwnd($w) {
    for ($i = 0; $i -lt 80; $i++) {
        if (Test-Path $w.HandleFile) {
            $lines = @(Get-Content $w.HandleFile)
            if ($lines.Count -ge 2) {
                $w.Pid = [int]$lines[1].Trim()
                if (-not ($mine -contains $w.Pid)) { [void]$mine.Add($w.Pid) }
                return [IntPtr][int64]$lines[0].Trim()
            }
        }
        Start-Sleep -Milliseconds 250
    }
    throw "no window announced itself"
}function Wait-Front($w, $seconds) {
    # A new window of ours takes the foreground by itself. Check that it really did before going on:
    # if the foreground is not our window, stop (never act on a window that is not ours).
    $h = Read-Hwnd $w
    $ok = $false
    for ($i = 0; $i -lt 40 -and -not $ok; $i++) {
        $ok = ([W]::GetForegroundWindow() -eq $h)
        if (-not $ok) { Start-Sleep -Milliseconds 250 }
    }
    if (-not $ok) {
        [W]::SwitchToThisWindow($h, $true)
        Start-Sleep -Milliseconds 500
        $ok = ([W]::GetForegroundWindow() -eq $h)
    }
    if (-not $ok) { throw "could not confirm that our own window is in front; stopping" }
    Start-Sleep -Seconds $seconds
}
function Close-Window($w) {
    # Closing the front window brings the one below it forward again.
    Stop-Process -Id $w.Pid -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 800
}
$exe2 = $null
function Tracker($args2) { return (& (Join-Path $root "Timelyx.exe") --ipc $args2 | Out-String).Trim() }

$daemon = $null
try {
    $daemon = Start-Mine (Join-Path $root "Timelyx.exe") "--background"
    Start-Sleep -Seconds 3
    Write-Host "tracker state: $(Tracker 'state')"

    if ($MeasureMinutes -le 0) {
        $a = Open-Window "chrome.exe" @("Learn Rust - YouTube - Google Chrome", "owner/repo - GitHub - Google Chrome") 9
        Wait-Front $a 6                    # YouTube; 9 s after it is shown the title changes to GitHub, still in front
        Start-Sleep -Seconds 7
        $b = Open-Window "Code.exe" @("main.rs - Visual Studio Code") 0
        Wait-Front $b 6                    # the editor
        $c = Open-Window "chrome.exe" @("Private plans - Google Chrome (Incognito)") 0
        Wait-Front $c 5                    # a private window
        Close-Window $c
        Wait-Front $b 4                    # back to the editor
        Write-Host "pause 15 min through the tracker, then resume"
        [void](Tracker 'pause 900')
        Start-Sleep -Seconds 4
        Write-Host ("while paused: " + (Tracker 'state'))
        [void](Tracker 'resume')
        Close-Window $b
        Wait-Front $a 5                    # back to the first window
        [void](Tracker 'flush')
    } else {
        $a = Open-Window "chrome.exe" @("Learn Rust - YouTube - Google Chrome") 0
        Wait-Front $a 2
        $d = Get-Process -Id $daemon.Id
        Start-Sleep -Seconds 20
        $d.Refresh()
        $cpu0 = $d.TotalProcessorTime.TotalSeconds; $t0 = Get-Date
        $samples = @()
        $end = $t0.AddMinutes($MeasureMinutes)
        $nextSwitch = $t0.AddSeconds(120)
        while ((Get-Date) -lt $end) {
            Start-Sleep -Seconds 30
            $d.Refresh()
            $samples += [pscustomobject]@{ t = [int]((Get-Date) - $t0).TotalSeconds; cpu = $d.TotalProcessorTime.TotalSeconds; ws = $d.WorkingSet64 / 1MB; priv = $d.PrivateMemorySize64 / 1MB }
            if ((Get-Date) -ge $nextSwitch) {
                # Two focus events: a fresh window of ours comes to the front, then goes away.
                $x = Open-Window "Code.exe" @("main.rs - Visual Studio Code") 0
                Wait-Front $x 2
                Close-Window $x
                $nextSwitch = (Get-Date).AddSeconds(120)
            }
        }
        $d.Refresh()
        $wall = ((Get-Date) - $t0).TotalSeconds
        $cpuUsed = $d.TotalProcessorTime.TotalSeconds - $cpu0
        $cores = [Environment]::ProcessorCount
        $maxWs = ($samples | Measure-Object ws -Maximum).Maximum
        $avgWs = ($samples | Measure-Object ws -Average).Average
        $maxPriv = ($samples | Measure-Object priv -Maximum).Maximum
        $lines = @(
            ("measured for {0:N0} s with no window open" -f $wall),
            ("cpu time used: {0:N3} s  ->  {1:N4} % of one core, {2:N4} % of all {3} cores" -f $cpuUsed, ($cpuUsed / $wall * 100), ($cpuUsed / $wall * 100 / $cores), $cores),
            ("working set: avg {0:N1} MB, max {1:N1} MB; private bytes max {2:N1} MB" -f $avgWs, $maxWs, $maxPriv)
        )
        $lines | ForEach-Object { Write-Host $_ }
        if ($Report) { $lines | Set-Content -Encoding utf8 $Report }
        [void](Tracker 'flush')
    }
    # Stop the tracker through its own channel, then read what it recorded.
    & (Join-Path $root "Timelyx.exe") --ipc quit | Out-Null
    Start-Sleep -Seconds 2
    $py = Join-Path $root "dump.py"
    @'
import sqlite3, sys, os
db = sqlite3.connect(os.path.join(sys.argv[1], "timelyx.db"))
rows = db.execute("""select s.start, s.end, a.exe, a.name, a.category, st.name, st.category
  from sessions s join apps a on a.id=s.app_id left join sites st on st.id=s.site_id order by s.start""").fetchall()
for r in rows:
    print(f"{r[1]-r[0]:>4}s  {r[3]:<20} {str(r[5]):<18} app={r[4]:<9} site={r[6]}  exe={os.path.basename(r[2])}")
print("apps:", [x[0] for x in db.execute("select exe from apps")])
print("sites:", [x[0] for x in db.execute("select name from sites")])
'@ | Set-Content -Encoding utf8 $py
    python -I $py $data
}
finally {
    Stop-Mine
    Start-Sleep -Milliseconds 500
    Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}

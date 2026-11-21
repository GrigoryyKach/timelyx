# A test window for tools/live-test.ps1: a small form with a title that can change on a schedule.
# It is run from a copy of powershell.exe renamed to chrome.exe or Code.exe, so Timelyx classifies it by name.
#   -TitleList "first title|second title" -At 9   (9 s after the window is shown its title becomes the second one)
param(
    [string]$TitleList,
    [int]$At = 0,
    [int]$Lifetime = 120,
    [string]$HandleFile = ""
)
$Titles = $TitleList.Split('|')
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public class S {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
$f = New-Object Windows.Forms.Form
$f.Text = $Titles[0]
$f.Width = 420
$f.Height = 160
$f.StartPosition = 'Manual'
$f.Location = New-Object Drawing.Point(120, 120)
$f.ShowInTaskbar = $true
$f.Add_Shown({
    # The first ShowWindow of a process takes its show state from how it was started (hidden, here); show it for real.
    [void][S]::ShowWindow($f.Handle, 5); [void][S]::ShowWindow($f.Handle, 5)
    $f.Activate(); [void][S]::SetForegroundWindow($f.Handle)
    $script:start = Get-Date
    if ($HandleFile) { "$($f.Handle.ToInt64())`n$PID" | Set-Content -Encoding ascii $HandleFile } })
$script:start = Get-Date
$t = New-Object Windows.Forms.Timer
$t.Interval = 500
$t.Add_Tick({
    $s = ((Get-Date) - $script:start).TotalSeconds
    if ($At -gt 0 -and $Titles.Count -gt 1 -and $s -ge $At) { $f.Text = $Titles[1] }
    if ($s -ge $Lifetime) { $f.Close() }
})
$t.Start()
[Windows.Forms.Application]::Run($f)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$artifactRoot = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts\windows'))
$testRoot = Join-Path $artifactRoot ('window-state-smoke-' + [guid]::NewGuid().ToString('N'))
$previousDataRoot = $env:MYTHIC_LOOT_DATA_DIR
$activeProcess = $null
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GeometryProbe {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
    [StructLayout(LayoutKind.Sequential)] public struct PLACEMENT { public uint Length,Flags,Show; public POINT Min,Max; public RECT Normal; }
    [StructLayout(LayoutKind.Sequential)] public struct MONITOR { public uint Size; public RECT Bounds,Work; public uint Flags; }
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetWindowPlacement(IntPtr window, ref PLACEMENT value);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr window, int x, int y, int width, int height, bool repaint);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr window, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Auto)] public static extern bool GetMonitorInfo(IntPtr monitor, ref MONITOR value);
    public static PLACEMENT Read(IntPtr window) {
        var value = new PLACEMENT { Length = (uint)Marshal.SizeOf(typeof(PLACEMENT)) };
        if (!GetWindowPlacement(window, ref value)) throw new Exception("GetWindowPlacement failed");
        return value;
    }
    public static MONITOR Screen(IntPtr window) {
        var value = new MONITOR { Size = (uint)Marshal.SizeOf(typeof(MONITOR)) };
        if (!GetMonitorInfo(MonitorFromWindow(window, 2), ref value)) throw new Exception("GetMonitorInfo failed");
        return value;
    }
}
'@
$oldDpi = [GeometryProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))

function Start-Probe([string]$Executable) {
    $script:activeProcess = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 200
        $script:activeProcess.Refresh()
        if ($script:activeProcess.HasExited) { throw 'Packaged app exited during geometry startup' }
    } while ($script:activeProcess.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)
    if ($script:activeProcess.MainWindowHandle -eq 0) { throw 'Packaged app has no visible window' }
    Start-Sleep -Milliseconds 600
    return $script:activeProcess.MainWindowHandle
}
function Close-Probe {
    $deadline = [DateTime]::UtcNow.AddSeconds(40)
    do {
        [void]$script:activeProcess.CloseMainWindow()
        if ($script:activeProcess.WaitForExit(500)) { break }
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $script:activeProcess.HasExited) { throw 'Packaged app did not close cleanly' }
    $script:activeProcess.Dispose()
    $script:activeProcess = $null
}
function Write-TestJson([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
}
function Get-TestHash([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)) }
    finally { $algorithm.Dispose(); $stream.Dispose() }
}
function Assert-NormalBounds($Expected, $Actual) {
    foreach ($field in @('Left','Top','Right','Bottom')) {
        if ([Math]::Abs($Expected.$field - $Actual.$field) -gt 2) { throw "Normal bounds drifted at ${field}: expected $($Expected | ConvertTo-Json -Compress), actual $($Actual | ConvertTo-Json -Compress)" }
    }
}
try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    foreach ($edition in @('Player','Developer')) {
        $exe = Join-Path $artifactRoot ($edition.ToLower() + '\win-unpacked\Mythic Loot Launcher ' + $edition + '.exe')
        $data = Join-Path $testRoot $edition
        New-Item -ItemType Directory -Path $data | Out-Null
        $env:MYTHIC_LOOT_DATA_DIR = $data
        $configPath = Join-Path $data 'launcher-config.json'
        $statePath = Join-Path $data 'window-state.json'
        $handle = Start-Probe $exe
        Close-Probe
        if (-not (Test-Path -LiteralPath $statePath)) { throw "$edition did not persist window placement" }
        $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        $config.preferences.autoCheckUpdates = $false
        $config.preferences.rememberWindow = $true
        Write-TestJson $configPath $config
        $handle = Start-Probe $exe
        $monitor = [GeometryProbe]::Screen($handle)
        $scale = [GeometryProbe]::GetDpiForWindow($handle) / 96.0
        $width = [Math]::Min([int](1100 * $scale), $monitor.Work.Right - $monitor.Work.Left)
        $height = [Math]::Min([int](700 * $scale), $monitor.Work.Bottom - $monitor.Work.Top)
        [void][GeometryProbe]::MoveWindow($handle, $monitor.Work.Left, $monitor.Work.Top, $width, $height, $true)
        Start-Sleep -Milliseconds 600
        $normal = [GeometryProbe]::Read($handle)
        Close-Probe
        $handle = Start-Probe $exe
        $restored = [GeometryProbe]::Read($handle)
        Assert-NormalBounds $normal.Normal $restored.Normal
        if ($restored.Show -ne 1) { throw "$edition restored a non-normal initial window" }
        [void][GeometryProbe]::ShowWindow($handle, 3)
        Start-Sleep -Milliseconds 600
        Close-Probe
        $saved = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        if (-not $saved.maximized) { throw "$edition did not save maximized state" }
        $handle = Start-Probe $exe
        $restored = [GeometryProbe]::Read($handle)
        if ($restored.Show -ne 3) { throw "$edition did not restore maximized state" }
        Assert-NormalBounds $normal.Normal $restored.Normal
        [void][GeometryProbe]::ShowWindow($handle, 6)
        Start-Sleep -Milliseconds 600
        Close-Probe
        $handle = Start-Probe $exe
        if ([GeometryProbe]::Read($handle).Show -ne 3) { throw "$edition reopened minimized or lost pre-minimize maximization" }
        Close-Probe
        $stateHash = Get-TestHash $statePath
        $config.preferences.rememberWindow = $false
        Write-TestJson $configPath $config
        $handle = Start-Probe $exe
        if ([GeometryProbe]::Read($handle).Show -ne 1) { throw "$edition ignored disabled placement preference" }
        Close-Probe
        if ((Get-TestHash $statePath) -ne $stateHash) { throw "$edition wrote placement while disabled" }
        $config.preferences.rememberWindow = $true
        Write-TestJson $configPath $config
        $saved.x = 900000; $saved.y = 900000; $saved.maximized = $false
        Write-TestJson $statePath $saved
        $handle = Start-Probe $exe
        $actual = [GeometryProbe]::Read($handle).Normal
        $monitor = [GeometryProbe]::Screen($handle)
        $screenX = $actual.Left + $monitor.Work.Left - $monitor.Bounds.Left
        $screenY = $actual.Top + $monitor.Work.Top - $monitor.Bounds.Top
        if ($screenX -lt $monitor.Work.Left -or $screenX -ge $monitor.Work.Right -or $screenY -lt $monitor.Work.Top -or $screenY -ge $monitor.Work.Bottom) { throw "$edition remained off-screen" }
        Close-Probe
        [IO.File]::WriteAllText($statePath, '{broken-window-state', [Text.UTF8Encoding]::new($false))
        $configHash = Get-TestHash $configPath
        $handle = Start-Probe $exe
        Close-Probe
        if ((Get-TestHash $configPath) -ne $configHash) { throw "$edition changed profile settings during placement recovery" }
        if (@(Get-ChildItem -LiteralPath $data -Filter 'window-state.invalid-*.json').Count -ne 1) { throw "$edition did not preserve malformed placement" }
        Write-Host "$edition passed normal/maximized/minimized restart, opt-out, off-screen recovery and malformed-state isolation."
    }
}
finally {
    if ($null -ne $activeProcess) {
        if (-not $activeProcess.HasExited) { Stop-Process -Id $activeProcess.Id -Force; $activeProcess.WaitForExit() }
        $activeProcess.Dispose()
    }
    $env:MYTHIC_LOOT_DATA_DIR = $previousDataRoot
    [void][GeometryProbe]::SetThreadDpiAwarenessContext($oldDpi)
    $resolved = [IO.Path]::GetFullPath($testRoot)
    if (-not $resolved.StartsWith($artifactRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Smoke cleanup escaped artifacts' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}

param(
    [string]$FromVersion = "0.1.0",
    [string]$ExpectedVersion = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($ExpectedVersion)) {
    $ExpectedVersion = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.conf.json") | ConvertFrom-Json).version
}
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot "artifacts\windows"))
$baseline = Join-Path $artifactRoot "release-acceptance\v$FromVersion\Mythic-Loot-Launcher-Player.exe"
$feedUrl = "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/latest/download/launcher-update-player.json"
$smokeRoot = Join-Path $artifactRoot ("live-app-update-smoke-" + [guid]::NewGuid().ToString("N"))
$installRoot = Join-Path $smokeRoot "install"
$dataRoot = Join-Path $smokeRoot "data"
$target = Join-Path $installRoot "Mythic Loot Launcher Player.exe"
$driver = Join-Path $PSScriptRoot "Drive-LiveAppUpdate.mjs"
$playerProcess = $null
$restartProcess = $null

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][string]$Path)

    $hashStream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [System.BitConverter]::ToString($algorithm.ComputeHash($hashStream)).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
        $hashStream.Dispose()
    }
}

function Stop-TestProcess {
    param([System.Diagnostics.Process]$Process)

    if ($null -eq $Process -or $Process.HasExited) {
        return
    }
    [void]$Process.CloseMainWindow()
    if (-not $Process.WaitForExit(5000)) {
        $Process.Kill()
        $Process.WaitForExit()
    }
}

if (-not (Test-Path -LiteralPath $baseline -PathType Leaf)) {
    throw "Preserved Player $FromVersion baseline was not found at $baseline"
}
if (-not (Test-Path -LiteralPath $driver -PathType Leaf)) {
    throw "Live update WebView driver was not found at $driver"
}

$feed = Invoke-RestMethod -Uri $feedUrl -Method Get -TimeoutSec 30
if ($feed.schemaVersion -ne 1 -or $feed.product -ne "Mythic Loot Launcher" -or $feed.edition -ne "player" -or $feed.version -ne $ExpectedVersion) {
    throw "The live Player feed is not the expected $ExpectedVersion release"
}
if ($feed.asset.fileName -ne "Mythic-Loot-Launcher-Player.exe" -or
    $feed.asset.url -ne "https://github.com/HixxyDubz/Mythic-Loot-Launcher-Rust-/releases/download/v$ExpectedVersion/Mythic-Loot-Launcher-Player.exe" -or
    $feed.asset.sha256 -notmatch '^[a-fA-F0-9]{64}$') {
    throw "The live Player feed asset contract is invalid"
}

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$debugPort = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()

New-Item -ItemType Directory -Path $installRoot, $dataRoot | Out-Null
try {
    Copy-Item -LiteralPath $baseline -Destination $target
    $baselineHash = Get-Sha256Hex -Path $target
    $env:MYTHIC_LOOT_DATA_DIR = $dataRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$debugPort"
    $playerProcess = Start-Process -FilePath $target -ArgumentList "--live-update-acceptance" -PassThru -WindowStyle Hidden

    $endpointDeadline = [DateTime]::UtcNow.AddSeconds(30)
    $page = $null
    do {
        Start-Sleep -Milliseconds 250
        try {
            $pages = @(Invoke-RestMethod -Uri "http://127.0.0.1:$debugPort/json" -TimeoutSec 2)
            $page = $pages | Where-Object { $_.type -eq "page" -and $_.webSocketDebuggerUrl } | Select-Object -First 1
        }
        catch {
            $page = $null
        }
    } while ($null -eq $page -and -not $playerProcess.HasExited -and [DateTime]::UtcNow -lt $endpointDeadline)
    if ($null -eq $page) {
        throw "The preserved Player did not expose its isolated WebView for acceptance"
    }

    $driverResult = & node.exe $driver $page.webSocketDebuggerUrl $ExpectedVersion $feed.asset.sha256
    if ($LASTEXITCODE -ne 0) {
        throw "The live Player update WebView driver failed with exit code $LASTEXITCODE"
    }
    $driverState = $driverResult | ConvertFrom-Json
    if (-not $driverState.reviewed -or -not $driverState.downloaded -or -not $driverState.confirmed) {
        throw "The live Player update UI did not complete its reviewed confirmation flow"
    }

    if (-not $playerProcess.WaitForExit(30000)) {
        $activityPath = Join-Path $dataRoot "activity-history.json"
        $activityDetails = if (Test-Path -LiteralPath $activityPath -PathType Leaf) {
            Get-Content -Raw -LiteralPath $activityPath
        }
        else {
            "No activity journal was written."
        }
        throw "Player $FromVersion did not close after confirmed live update. Native activity: $activityDetails"
    }
    $resultPath = Join-Path $dataRoot "app-update-last-result.json"
    $resultDeadline = [DateTime]::UtcNow.AddSeconds(60)
    while (-not (Test-Path -LiteralPath $resultPath -PathType Leaf) -and [DateTime]::UtcNow -lt $resultDeadline) {
        Start-Sleep -Milliseconds 250
    }
    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw "The live update helper did not write its result journal"
    }
    $result = Get-Content -Raw -LiteralPath $resultPath | ConvertFrom-Json
    if (-not $result.success -or $result.version -ne $ExpectedVersion -or -not $result.restartProcessId) {
        throw "The live update result did not report a successful $ExpectedVersion replacement and restart"
    }
    if ((Get-Item -LiteralPath $target).Length -ne [long]$feed.asset.bytes -or (Get-Sha256Hex -Path $target) -ne $feed.asset.sha256.ToLowerInvariant()) {
        throw "The installed Player does not match the live feed's size and SHA-256"
    }
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $dataRoot "app-update-staging") -Filter "player.previous.exe" -File -Recurse)
    if ($backups.Count -ne 1 -or (Get-Sha256Hex -Path $backups[0].FullName) -ne $baselineHash) {
        throw "The live update did not retain one exact Player $FromVersion backup"
    }

    $restartProcess = [System.Diagnostics.Process]::GetProcessById([int]$result.restartProcessId)
    $windowDeadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 250
        $restartProcess.Refresh()
    } while (-not $restartProcess.HasExited -and $restartProcess.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $windowDeadline)
    if ($restartProcess.HasExited -or $restartProcess.MainWindowHandle -eq 0 -or $restartProcess.MainWindowTitle -ne "Mythic Loot Launcher Player") {
        throw "Updated Player $ExpectedVersion did not restart into its responsive window"
    }

    Write-Host "Live Player update passed: $FromVersion -> $ExpectedVersion through the real GitHub feed, WebView review/download/confirmation, verified replacement, exact backup, result journal and responsive restart."
}
finally {
    Stop-TestProcess -Process $restartProcess
    Stop-TestProcess -Process $playerProcess
    Remove-Item Env:MYTHIC_LOOT_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
    if (-not $resolvedSmokeRoot.StartsWith($artifactRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -or
        [System.IO.Path]::GetFileName($resolvedSmokeRoot) -notmatch '^live-app-update-smoke-[0-9a-f]{32}$') {
        throw "Live app-update smoke cleanup target escaped the Windows artifacts directory: $resolvedSmokeRoot"
    }
    if (Test-Path -LiteralPath $resolvedSmokeRoot -PathType Container) {
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
    }
}

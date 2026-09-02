param(
    [Parameter(Mandatory = $true)][string]$ReleaseNotes,
    [switch]$ConfirmPublish
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $ConfirmPublish) {
    throw "Live GitHub publication requires -ConfirmPublish"
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot "artifacts\windows"))
$manifestPath = Join-Path $artifactRoot "build-manifest.json"
$developerExecutable = Join-Path $artifactRoot "developer\win-unpacked\Mythic Loot Launcher Developer.exe"
$driver = Join-Path $PSScriptRoot "Drive-LiveAppRelease.mjs"
$version = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.conf.json") | ConvertFrom-Json).version
$tag = "v$version"
$smokeRoot = Join-Path $artifactRoot ("live-app-release-smoke-" + [guid]::NewGuid().ToString("N"))
$dataRoot = Join-Path $smokeRoot "data"
$developerProcess = $null

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or -not (Test-Path -LiteralPath $developerExecutable -PathType Leaf)) {
    throw "Build both Windows editions before publishing the Player app"
}
$build = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
if ($build.version -ne $version -or @($build.editions) -notcontains "player" -or @($build.editions) -notcontains "developer") {
    throw "The Windows build manifest does not match Developer $version with both editions"
}
$playerArtifacts = @($build.artifacts | Where-Object { $_.edition -eq "player" })
if ($playerArtifacts.Count -ne 2) {
    throw "The build manifest must contain exactly two Player artifacts"
}
$expectedHashes = @{}
foreach ($artifact in $playerArtifacts) {
    $expectedHashes[$artifact.kind] = $artifact.sha256.ToLowerInvariant()
}

& gh.exe auth status --hostname github.com 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "GitHub CLI is not authenticated"
}
& gh.exe release view $tag --repo HixxyDubz/Mythic-Loot-Launcher-Rust- 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    throw "GitHub release $tag already exists"
}

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$debugPort = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()

New-Item -ItemType Directory -Path $dataRoot | Out-Null
try {
    $env:MYTHIC_LOOT_DATA_DIR = $dataRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$debugPort"
    $developerProcess = Start-Process -FilePath $developerExecutable -ArgumentList "--live-app-release-acceptance" -PassThru -WindowStyle Hidden
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
    } while ($null -eq $page -and -not $developerProcess.HasExited -and [DateTime]::UtcNow -lt $endpointDeadline)
    if ($null -eq $page) {
        throw "The Developer app did not expose its isolated WebView for release acceptance"
    }

    $hashJson = $expectedHashes | ConvertTo-Json -Compress
    $driverResult = & node.exe $driver $page.webSocketDebuggerUrl $version $manifestPath $ReleaseNotes $hashJson
    if ($LASTEXITCODE -ne 0) {
        throw "The Developer app-release WebView driver failed with exit code $LASTEXITCODE"
    }
    $driverState = $driverResult | ConvertFrom-Json
    if (-not $driverState.reviewed -or -not $driverState.confirmed -or -not $driverState.published) {
        throw "The visible Developer publication flow did not complete"
    }

    $release = & gh.exe release view $tag --repo HixxyDubz/Mythic-Loot-Launcher-Rust- --json tagName,url,isLatest,assets | ConvertFrom-Json
    $assetNames = @($release.assets | ForEach-Object { $_.name })
    $expectedNames = @("Mythic-Loot-Launcher-Player.exe", "Mythic-Loot-Launcher-Player-Setup.exe", "launcher-update-player.json")
    if ($release.tagName -ne $tag -or -not $release.isLatest -or $assetNames.Count -ne 3 -or @($expectedNames | Where-Object { $assetNames -notcontains $_ }).Count -ne 0) {
        throw "GitHub release $tag does not contain the exact latest Player asset set"
    }
    Write-Host "Developer live publication passed: reviewed both packaged Player hashes, confirmed in the visible UI, and published exact latest release $tag at $($release.url)."
}
finally {
    if ($null -ne $developerProcess -and -not $developerProcess.HasExited) {
        [void]$developerProcess.CloseMainWindow()
        if (-not $developerProcess.WaitForExit(5000)) {
            $developerProcess.Kill()
            $developerProcess.WaitForExit()
        }
    }
    Remove-Item Env:MYTHIC_LOOT_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
    if (-not $resolvedSmokeRoot.StartsWith($artifactRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -or
        [System.IO.Path]::GetFileName($resolvedSmokeRoot) -notmatch '^live-app-release-smoke-[0-9a-f]{32}$') {
        throw "Live app-release cleanup target escaped the Windows artifacts directory: $resolvedSmokeRoot"
    }
    if (Test-Path -LiteralPath $resolvedSmokeRoot -PathType Container) {
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
    }
}

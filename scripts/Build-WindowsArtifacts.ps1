param(
    [ValidateSet("All", "Player", "Developer")]
    [string]$Edition = "All"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$baseConfigPath = Join-Path $projectRoot "src-tauri\tauri.conf.json"
$baseConfig = Get-Content -Raw -LiteralPath $baseConfigPath | ConvertFrom-Json
$targetRelease = Join-Path $projectRoot "src-tauri\target\release"
$nativeExecutable = Join-Path $targetRelease "mythic-loot-launcher.exe"
$bundleDirectory = Join-Path $targetRelease "bundle\nsis"
$artifactDirectory = Join-Path $projectRoot "artifacts\windows"
$mockDataCheck = Join-Path $projectRoot "scripts\Verify-NoProductionMockData.ps1"

$allFlavors = @(
    [pscustomobject]@{
        Key = "player"
        Label = "Player"
        ProductName = "Mythic Loot Launcher Player"
        ConfigPath = Join-Path $projectRoot "src-tauri\tauri.player.conf.json"
        CargoFeatures = "player"
    },
    [pscustomobject]@{
        Key = "developer"
        Label = "Developer"
        ProductName = "Mythic Loot Launcher Developer"
        ConfigPath = Join-Path $projectRoot "src-tauri\tauri.developer.conf.json"
        CargoFeatures = "developer"
    }
)
$flavors = if ($Edition -eq "All") {
    $allFlavors
}
else {
    @($allFlavors | Where-Object Label -eq $Edition)
}

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][string]$Path)

    $stream = [System.IO.File]::OpenRead($Path)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [System.BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace("-", "")
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
}

function Assert-ArtifactChild {
    param([Parameter(Mandatory = $true)][string]$Path)

    $fullArtifactDirectory = [System.IO.Path]::GetFullPath($artifactDirectory)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($fullArtifactDirectory + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Artifact path escaped the project artifacts directory: $fullPath"
    }
}

$buildManifestPath = Join-Path $artifactDirectory "build-manifest.json"
$artifactRecords = @()
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $mockDataCheck
if ($LASTEXITCODE -ne 0) {
    throw "Production mock-data source verification failed."
}
foreach ($flavor in $flavors) {
    if (-not (Test-Path -LiteralPath $flavor.ConfigPath -PathType Leaf)) {
        throw "$($flavor.Label) Tauri configuration was not found at $($flavor.ConfigPath)"
    }

    $tauriArguments = @(
        "run", "tauri", "build", "--",
        "--bundles", "nsis",
        "--config", $flavor.ConfigPath
    )
    if ($flavor.CargoFeatures -eq "player") {
        $tauriArguments += @("--", "--no-default-features")
    }
    else {
        $tauriArguments += @("--features", "developer")
    }

    Write-Host "Building the $($flavor.Label) edition..."
    Push-Location $projectRoot
    try {
        & npm.cmd @tauriArguments
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri returned exit code $LASTEXITCODE while building the $($flavor.Label) edition."
        }
    }
    finally {
        Pop-Location
    }

    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $mockDataCheck -IncludeDist
    if ($LASTEXITCODE -ne 0) {
        throw "$($flavor.Label) frontend mock-data verification failed."
    }

    if ($flavor.Key -eq "player") {
        $forbiddenPlayerText = @(
            "Add a modpack",
            "DEVELOPER WORKSPACE",
            "Developer source folder",
            "Distribution channel",
            "Manifest URL",
            "create_github_repository",
            "publish_modpack_release",
            "prepare_public_catalog",
            "publish_public_catalog",
            "save_manifest_content",
            "prepare_manifest_content_release",
            "publish_manifest_content_release",
            "prepare_player_app_release",
            "publish_player_app_release",
            "Manifest content editor",
            "Content-only manifest release",
            "Player public catalogue",
            "Publish the public Player app",
            "Windows build manifest"
        )
        $playerScripts = @(Get-ChildItem -LiteralPath (Join-Path $projectRoot "dist\assets") -Filter "*.js" -File)
        foreach ($forbidden in $forbiddenPlayerText) {
            if ($playerScripts | Select-String -SimpleMatch -Pattern $forbidden -Quiet) {
                throw "Player frontend contains Developer-only text or IPC identifier: $forbidden"
            }
        }
    }

    if (-not (Test-Path -LiteralPath $nativeExecutable -PathType Leaf)) {
        throw "The $($flavor.Label) native executable was not produced at $nativeExecutable"
    }

    $installer = Get-ChildItem -LiteralPath $bundleDirectory -Filter "$($flavor.ProductName)*.exe" -File |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if (-not $installer) {
        throw "The $($flavor.Label) NSIS installer was not produced in $bundleDirectory"
    }

    $flavorDirectory = Join-Path $artifactDirectory $flavor.Key
    $unpackedDirectory = Join-Path $flavorDirectory "win-unpacked"
    $unpackedExecutable = Join-Path $unpackedDirectory "$($flavor.ProductName).exe"
    $installerOutput = Join-Path $flavorDirectory "$($flavor.ProductName) Setup $($baseConfig.version).exe"
    Assert-ArtifactChild -Path $flavorDirectory
    Assert-ArtifactChild -Path $unpackedDirectory
    Assert-ArtifactChild -Path $unpackedExecutable
    Assert-ArtifactChild -Path $installerOutput

    New-Item -ItemType Directory -Path $unpackedDirectory -Force | Out-Null
    Copy-Item -LiteralPath $nativeExecutable -Destination $unpackedExecutable -Force
    Copy-Item -LiteralPath $installer.FullName -Destination $installerOutput -Force

    foreach ($artifact in @(
        [pscustomobject]@{ Kind = "installer"; Path = $installerOutput },
        [pscustomobject]@{ Kind = "win-unpacked"; Path = $unpackedExecutable }
    )) {
        $file = Get-Item -LiteralPath $artifact.Path
        $artifactRecords += [ordered]@{
            edition = $flavor.Key
            kind = $artifact.Kind
            path = $file.FullName
            bytes = $file.Length
            sha256 = Get-Sha256Hex -Path $file.FullName
        }
    }
}

if ($Edition -ne "All" -and (Test-Path -LiteralPath $buildManifestPath -PathType Leaf)) {
    $builtEditionKeys = @($flavors | ForEach-Object Key)
    $previousManifest = Get-Content -Raw -LiteralPath $buildManifestPath | ConvertFrom-Json
    foreach ($previousArtifact in @($previousManifest.artifacts)) {
        if ($previousArtifact.edition -notin $builtEditionKeys -and (Test-Path -LiteralPath $previousArtifact.path -PathType Leaf)) {
            $file = Get-Item -LiteralPath $previousArtifact.path
            $artifactRecords += [ordered]@{
                edition = $previousArtifact.edition
                kind = $previousArtifact.kind
                path = $file.FullName
                bytes = $file.Length
                sha256 = Get-Sha256Hex -Path $file.FullName
            }
        }
    }
}

New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null
$manifestEditions = @("player", "developer") | Where-Object { $_ -in @($artifactRecords | ForEach-Object edition) }
$buildManifest = [ordered]@{
    product = "Mythic Loot Launcher"
    version = $baseConfig.version
    editions = $manifestEditions
    createdAtUtc = (Get-Date).ToUniversalTime().ToString("o")
    artifacts = $artifactRecords
} | ConvertTo-Json -Depth 5
Set-Content -LiteralPath $buildManifestPath -Value $buildManifest -Encoding utf8

$legacyInstaller = Join-Path $artifactDirectory "Mythic Loot Launcher Setup $($baseConfig.version).exe"
$legacyUnpacked = Join-Path $artifactDirectory "win-unpacked"
Assert-ArtifactChild -Path $legacyInstaller
Assert-ArtifactChild -Path $legacyUnpacked
if (Test-Path -LiteralPath $legacyInstaller -PathType Leaf) {
    Remove-Item -LiteralPath $legacyInstaller -Force
}
if (Test-Path -LiteralPath $legacyUnpacked -PathType Container) {
    Remove-Item -LiteralPath $legacyUnpacked -Recurse -Force
}

Write-Host "Windows edition artifacts are ready:"
foreach ($artifact in $artifactRecords) {
    Write-Host "  $($artifact.edition) $($artifact.kind): $($artifact.path)"
}
Write-Host "  Hashes: $buildManifestPath"

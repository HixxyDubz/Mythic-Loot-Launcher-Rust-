$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$tauriConfigPath = Join-Path $projectRoot "src-tauri\tauri.conf.json"
$tauriConfig = Get-Content -Raw -LiteralPath $tauriConfigPath | ConvertFrom-Json
$targetRelease = Join-Path $projectRoot "src-tauri\target\release"
$nativeExecutable = Join-Path $targetRelease "mythic-loot-launcher.exe"
$bundleDirectory = Join-Path $targetRelease "bundle\nsis"
$artifactDirectory = Join-Path $projectRoot "artifacts\windows"
$unpackedDirectory = Join-Path $artifactDirectory "win-unpacked"
$unpackedExecutable = Join-Path $unpackedDirectory "$($tauriConfig.productName).exe"
$installerOutput = Join-Path $artifactDirectory "$($tauriConfig.productName) Setup $($tauriConfig.version).exe"

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

Push-Location $projectRoot
try {
    & npm.cmd run tauri build -- --bundles nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri returned exit code $LASTEXITCODE while building Windows artifacts."
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $nativeExecutable -PathType Leaf)) {
    throw "The native release executable was not produced at $nativeExecutable"
}

$installer = Get-ChildItem -LiteralPath $bundleDirectory -Filter "*.exe" -File |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $installer) {
    throw "The NSIS installer was not produced in $bundleDirectory"
}

New-Item -ItemType Directory -Path $unpackedDirectory -Force | Out-Null
Copy-Item -LiteralPath $nativeExecutable -Destination $unpackedExecutable -Force
Copy-Item -LiteralPath $installer.FullName -Destination $installerOutput -Force

$artifacts = @($installerOutput, $unpackedExecutable) | ForEach-Object {
    $file = Get-Item -LiteralPath $_
    [ordered]@{
        path = $file.FullName
        bytes = $file.Length
        sha256 = Get-Sha256Hex -Path $file.FullName
    }
}
$buildManifest = [ordered]@{
    product = $tauriConfig.productName
    version = $tauriConfig.version
    createdAtUtc = (Get-Date).ToUniversalTime().ToString("o")
    artifacts = $artifacts
} | ConvertTo-Json -Depth 5
$buildManifestPath = Join-Path $artifactDirectory "build-manifest.json"
Set-Content -LiteralPath $buildManifestPath -Value $buildManifest -Encoding utf8

Write-Host "Windows artifacts are ready:"
Write-Host "  Installer:    $installerOutput"
Write-Host "  Win unpacked: $unpackedExecutable"
Write-Host "  Hashes:       $buildManifestPath"

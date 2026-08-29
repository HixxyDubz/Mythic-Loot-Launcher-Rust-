param(
    [switch]$IncludeDist
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$sourceRoot = Join-Path $projectRoot "src"
$legacyMockModule = Join-Path $sourceRoot "mock.ts"
$forbiddenTerms = @(
    "previewPayload",
    "previewProfiles",
    "previewHealth",
    "Browser preview",
    "native persistence is available",
    "test/fixtures",
    "test-only data directory",
    "test input"
)

if (Test-Path -LiteralPath $legacyMockModule) {
    throw "Production mock module still exists: $legacyMockModule"
}

$productionSources = Get-ChildItem -LiteralPath $sourceRoot -Recurse -File |
    Where-Object {
        $_.Extension -in @(".ts", ".tsx", ".js", ".jsx") -and
        $_.Name -notmatch "\.test\." -and
        $_.FullName -notlike "*\src\test\*"
    }
$sourceMatches = $productionSources | Select-String -SimpleMatch -Pattern $forbiddenTerms
if ($sourceMatches) {
    $details = ($sourceMatches | ForEach-Object { "$($_.Path):$($_.LineNumber): $($_.Line.Trim())" }) -join [Environment]::NewLine
    throw "Production preview data references remain:$([Environment]::NewLine)$details"
}

if ($IncludeDist) {
    $distRoot = Join-Path $projectRoot "dist"
    if (-not (Test-Path -LiteralPath $distRoot -PathType Container)) {
        throw "Frontend output was not found at $distRoot"
    }
    $bundleFiles = Get-ChildItem -LiteralPath $distRoot -Recurse -File
    $bundleMatches = $bundleFiles | Select-String -SimpleMatch -Pattern $forbiddenTerms
    if ($bundleMatches) {
        $details = ($bundleMatches | ForEach-Object { "$($_.Path): $($_.Pattern)" }) -join [Environment]::NewLine
        throw "Built frontend contains preview data:$([Environment]::NewLine)$details"
    }
}

Write-Host "Production mock-data check passed$(if ($IncludeDist) { ' for source and built frontend' } else { ' for source' })."

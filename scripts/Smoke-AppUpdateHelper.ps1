param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$appVersion = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.conf.json") | ConvertFrom-Json).version
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot "artifacts\windows"))
$playerExecutable = Join-Path $artifactRoot "player\win-unpacked\Mythic Loot Launcher Player.exe"
$smokeRoot = Join-Path $artifactRoot ("app-update-smoke-" + [guid]::NewGuid().ToString("N"))
$dataRoot = Join-Path $smokeRoot "data"
$stageRoot = Join-Path $dataRoot "app-update-staging\fixture"
$installRoot = Join-Path $smokeRoot "install"
$target = Join-Path $installRoot "Mythic Loot Launcher Player.exe"
$staged = Join-Path $stageRoot "player.next.exe"
$helper = Join-Path $stageRoot "mythic-restart-agent.exe"
$backup = Join-Path $stageRoot "player.previous.exe"
$journalPath = Join-Path $stageRoot "apply-update.json"
$resultPath = Join-Path $dataRoot "app-update-last-result.json"
$launchedProcesses = @()

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

function Get-ProcessIdsForExecutable {
    param([Parameter(Mandatory = $true)][string]$ExpectedPath)

    $fullExpectedPath = [System.IO.Path]::GetFullPath($ExpectedPath)
    $ids = @()
    foreach ($candidate in @(Get-Process -ErrorAction SilentlyContinue)) {
        try {
            if ($candidate.Path -and [string]::Equals([System.IO.Path]::GetFullPath($candidate.Path), $fullExpectedPath, [System.StringComparison]::OrdinalIgnoreCase)) {
                $ids += $candidate.Id
            }
        }
        catch {
            # Protected unrelated processes are outside this isolated fixture.
        }
    }
    return @($ids | Select-Object -Unique)
}

function Stop-And-WaitForProcessIds {
    param([int[]]$ProcessIds)

    foreach ($processIdentifier in $ProcessIds) {
        if (-not (Get-Process -Id $processIdentifier -ErrorAction SilentlyContinue)) {
            continue
        }
        & taskkill.exe /PID $processIdentifier /F 2>&1 | Out-Null
        $stopDeadline = [DateTime]::UtcNow.AddSeconds(8)
        while (Get-Process -Id $processIdentifier -ErrorAction SilentlyContinue) {
            if ([DateTime]::UtcNow -ge $stopDeadline) {
                throw "Temporary restarted Player process $processIdentifier did not stop"
            }
            Start-Sleep -Milliseconds 100
        }
    }
}

foreach ($stale in @(Get-ChildItem -LiteralPath $artifactRoot -Directory -Filter "app-update-smoke-*")) {
    $resolvedStale = [System.IO.Path]::GetFullPath($stale.FullName)
    if ($stale.Name -notmatch '^app-update-smoke-[0-9a-f]{32}$' -or
        -not $resolvedStale.StartsWith($artifactRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean an unexpected app-update smoke folder: $resolvedStale"
    }
    $staleTarget = Join-Path $resolvedStale "install\Mythic Loot Launcher Player.exe"
    Stop-And-WaitForProcessIds -ProcessIds @(Get-ProcessIdsForExecutable -ExpectedPath $staleTarget)
    Remove-Item -LiteralPath $resolvedStale -Recurse -Force
}

if (-not (Test-Path -LiteralPath $playerExecutable -PathType Leaf)) {
    throw "Packaged Player executable was not found at $playerExecutable"
}

New-Item -ItemType Directory -Path $stageRoot, $installRoot | Out-Null
try {
    Copy-Item -LiteralPath $playerExecutable -Destination $target
    Copy-Item -LiteralPath $playerExecutable -Destination $staged
    Copy-Item -LiteralPath $playerExecutable -Destination $helper

    $marker = [System.Text.Encoding]::UTF8.GetBytes("`r`nMYTHIC-LOOT-CONTROLLED-UPDATE-SMOKE`r`n")
    $stream = [System.IO.File]::Open($staged, [System.IO.FileMode]::Append, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try {
        $stream.Write($marker, 0, $marker.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }

    $originalHash = Get-Sha256Hex -Path $target
    $stagedHash = Get-Sha256Hex -Path $staged
    if ($originalHash -eq $stagedHash) {
        throw "Controlled staged Player executable did not differ from the installed fixture"
    }

    $journal = [ordered]@{
        schemaVersion = 1
        version = $appVersion
        targetExe = $target
        targetSha256 = $originalHash
        stagedExe = $staged
        stagedBytes = (Get-Item -LiteralPath $staged).Length
        stagedSha256 = $stagedHash
        backupExe = $backup
        resultPath = $resultPath
        restartProbe = $true
    }
    $journalJson = $journal | ConvertTo-Json
    [System.IO.File]::WriteAllText($journalPath, $journalJson, [System.Text.UTF8Encoding]::new($false))

    $env:MYTHIC_LOOT_DATA_DIR = $dataRoot
    # Match Rust's std::process::Command/CreateProcess path exactly. ShellExecute can
    # hide accidental UAC installer detection by elevating instead of returning 740.
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $helper
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.Arguments = "--mythic-loot-apply-update `"$journalPath`" 0"
    $helperProcess = [System.Diagnostics.Process]::Start($startInfo)
    if (-not $helperProcess.WaitForExit(15000)) {
        Stop-Process -Id $helperProcess.Id -Force -ErrorAction SilentlyContinue
        throw "Packaged update helper did not exit within 15 seconds"
    }
    if ($helperProcess.ExitCode -ne 0) {
        throw "Packaged update helper exited with code $($helperProcess.ExitCode)"
    }

    $resultDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while (-not (Test-Path -LiteralPath $resultPath -PathType Leaf) -and [DateTime]::UtcNow -lt $resultDeadline) {
        Start-Sleep -Milliseconds 200
    }
    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw "Packaged update helper did not write its result journal"
    }
    $result = Get-Content -Raw -LiteralPath $resultPath | ConvertFrom-Json
    if (-not $result.success) {
        throw "Packaged update helper reported failure: $($result.message)"
    }
    if ((Get-Sha256Hex -Path $target) -ne $stagedHash) {
        throw "Packaged update helper did not activate the reviewed staged bytes"
    }
    if ((Get-Sha256Hex -Path $backup) -ne $originalHash) {
        throw "Packaged update helper did not retain an exact previous-executable backup"
    }

    Start-Sleep -Milliseconds 750
    $launchedProcesses = @($result.restartProcessId)
    if ($launchedProcesses.Count -eq 0) {
        throw "Packaged update helper did not restart the replaced Player fixture"
    }
    $restartDeadline = [DateTime]::UtcNow.AddSeconds(8)
    while (Get-Process -Id $result.restartProcessId -ErrorAction SilentlyContinue) {
        if ([DateTime]::UtcNow -ge $restartDeadline) {
            throw "Packaged update restart probe did not exit"
        }
        Start-Sleep -Milliseconds 100
    }

    Write-Host "Packaged Player update helper replaced different bytes, preserved the exact backup, verified the activated SHA-256, recorded success and restarted Player."
}
finally {
    $remainingProcesses = @(Get-ProcessIdsForExecutable -ExpectedPath $target)
    Stop-And-WaitForProcessIds -ProcessIds @($launchedProcesses + $remainingProcesses | Select-Object -Unique)
    Remove-Item Env:MYTHIC_LOOT_DATA_DIR -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
    if (-not $resolvedSmokeRoot.StartsWith($artifactRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "App-update smoke cleanup target escaped the Windows artifacts directory: $resolvedSmokeRoot"
    }
    if (Test-Path -LiteralPath $resolvedSmokeRoot -PathType Container) {
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
    }
}

param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$projectRoot = Split-Path -Parent $PSScriptRoot
$artifactRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot "artifacts\windows"))
$smokeRoot = Join-Path $artifactRoot ("smoke-" + [guid]::NewGuid().ToString("N"))
$editions = @(
    [pscustomobject]@{
        Name = "Player"
        Executable = Join-Path $artifactRoot "player\win-unpacked\Mythic Loot Launcher Player.exe"
        ExpectedTitle = "Mythic Loot Launcher Player"
    },
    [pscustomobject]@{
        Name = "Developer"
        Executable = Join-Path $artifactRoot "developer\win-unpacked\Mythic Loot Launcher Developer.exe"
        ExpectedTitle = "Mythic Loot Launcher Developer"
    }
)

function Stop-SmokeProcess {
    param([Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process)

    if ($Process.HasExited) {
        return
    }
    [void]$Process.CloseMainWindow()
    if (-not $Process.WaitForExit(5000)) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit()
    }
}

$results = @()
New-Item -ItemType Directory -Path $smokeRoot | Out-Null
try {
    foreach ($edition in $editions) {
        if (-not (Test-Path -LiteralPath $edition.Executable -PathType Leaf)) {
            throw "$($edition.Name) portable executable was not found at $($edition.Executable)"
        }
        $dataRoot = Join-Path $smokeRoot $edition.Name
        New-Item -ItemType Directory -Path $dataRoot | Out-Null
        $env:MYTHIC_LOOT_DATA_DIR = $dataRoot
        $process = Start-Process -FilePath $edition.Executable -PassThru -WindowStyle Hidden
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(25)
            do {
                Start-Sleep -Milliseconds 250
                $process.Refresh()
            } while (-not $process.HasExited -and $process.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)

            if ($process.HasExited) {
                throw "$($edition.Name) portable exited with code $($process.ExitCode) before creating a window"
            }
            if ($process.MainWindowHandle -eq 0) {
                throw "$($edition.Name) portable did not create a main window"
            }
            if ($process.MainWindowTitle -ne $edition.ExpectedTitle) {
                throw "$($edition.Name) window title was '$($process.MainWindowTitle)', expected '$($edition.ExpectedTitle)'"
            }
            $configPath = Join-Path $dataRoot "launcher-config.json"
            $configDeadline = [DateTime]::UtcNow.AddSeconds(25)
            do {
                $configCreated = Test-Path -LiteralPath $configPath -PathType Leaf
                if (-not $configCreated) {
                    Start-Sleep -Milliseconds 250
                    $process.Refresh()
                }
            } while (-not $configCreated -and -not $process.HasExited -and [DateTime]::UtcNow -lt $configDeadline)
            if (-not $configCreated) {
                throw "$($edition.Name) did not create isolated launcher configuration"
            }
            $activityPath = Join-Path $dataRoot "activity-history.json"
            $activityDeadline = [DateTime]::UtcNow.AddSeconds(25)
            do {
                $activityCreated = Test-Path -LiteralPath $activityPath -PathType Leaf
                if (-not $activityCreated) {
                    Start-Sleep -Milliseconds 250
                    $process.Refresh()
                }
            } while (-not $activityCreated -and -not $process.HasExited -and [DateTime]::UtcNow -lt $activityDeadline)
            if (-not $activityCreated) {
                throw "$($edition.Name) did not record packaged native startup activity"
            }
            $results += [pscustomobject]@{
                Edition = $edition.Name
                Handle = $process.MainWindowHandle
                Title = $process.MainWindowTitle
                ConfigCreated = $configCreated
                ActivityCreated = $activityCreated
            }
        }
        finally {
            Stop-SmokeProcess -Process $process
        }
    }
}
finally {
    Remove-Item Env:MYTHIC_LOOT_DATA_DIR -ErrorAction SilentlyContinue
    $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
    if (-not $resolvedSmokeRoot.StartsWith($artifactRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Smoke cleanup target escaped the Windows artifacts directory: $resolvedSmokeRoot"
    }
    if (Test-Path -LiteralPath $resolvedSmokeRoot -PathType Container) {
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
    }
}

$results | Format-Table -AutoSize
Write-Host "Both portable Windows editions passed isolated responsive-window and native-activity smoke acceptance."

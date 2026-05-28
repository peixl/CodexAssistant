# Build CodexAssistant with the system MSVC toolchain.
# Loads vcvars64 from the local VS Build Tools install so cargo can find
# link.exe, then runs the project's debug build.
#
# Usage:
#   pwsh -File scripts\dev\build-msvc.ps1                    # debug build
#   pwsh -File scripts\dev\build-msvc.ps1 -Release           # release build
#   pwsh -File scripts\dev\build-msvc.ps1 -CargoArgs "test --workspace"

[CmdletBinding()]
param(
    [switch]$Release,
    [string]$CargoArgs = "build -p codex-assistant-launcher -p codex-assistant"
)

$ErrorActionPreference = "Stop"

# Resolve repo root from this script's location so the script is portable.
$RepoRoot = (Resolve-Path -Path (Join-Path $PSScriptRoot "..\..")).Path

# Locate vcvars64.bat via vswhere — survives different VS editions / years.
function Find-VcVars64 {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $installPath = & $vswhere -latest -products * `
            -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
            -property installationPath 2>$null
        if ($installPath) {
            $candidate = Join-Path $installPath 'VC\Auxiliary\Build\vcvars64.bat'
            if (Test-Path $candidate) { return $candidate }
        }
    }
    # Fallback: common BuildTools location.
    $fallback = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
    if (Test-Path $fallback) { return $fallback }
    return $null
}

$vcvars = Find-VcVars64
if (-not $vcvars) {
    Write-Error "vcvars64.bat not found. Install Visual Studio Build Tools with the 'Desktop development with C++' workload."
    exit 1
}

Write-Host "Loading MSVC environment from: $vcvars"
$tempFile = [System.IO.Path]::GetTempFileName()
try {
    cmd.exe /c "`"$vcvars`" >nul 2>&1 && set > `"$tempFile`""
    Get-Content $tempFile | ForEach-Object {
        if ($_ -match '^([^=]+)=(.*)$') {
            [System.Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
        }
    }
} finally {
    Remove-Item $tempFile -ErrorAction SilentlyContinue
}

$linkCmd = Get-Command link -ErrorAction SilentlyContinue
if ($linkCmd) { Write-Host "link.exe: $($linkCmd.Source)" } else { Write-Host "link.exe not on PATH (cargo may still locate it)" }

Set-Location $RepoRoot
$argsArray = $CargoArgs.Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)
if ($Release) { $argsArray += '--release' }
Write-Host "Running: cargo $($argsArray -join ' ')"
& cargo @argsArray
exit $LASTEXITCODE

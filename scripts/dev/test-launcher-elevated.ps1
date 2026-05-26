param(
    [string]$LauncherExe = "D:\Github\CodexAssistant\target\release\codex-assistant.exe",
    [string]$StatusPath = "$env:USERPROFILE\.codex-session-delete\latest-status.json",
    [int]$WaitSeconds = 25
)

$ErrorActionPreference = 'Stop'

$logDir = Join-Path $PSScriptRoot 'out'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = Join-Path $logDir 'test-launcher.log'
function Log($msg) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $msg
    Add-Content -Path $log -Value $line
    Write-Output $line
}
"=== run @ $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===" | Set-Content -Path $log

$rule = Get-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound' -ErrorAction SilentlyContinue
if ($rule) {
    Log "Disabling firewall rule codex_sandbox_offline_block_outbound (Enabled was $($rule.Enabled))"
    Disable-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound'
} else {
    Log "Firewall rule codex_sandbox_offline_block_outbound not present"
}

$statusBackup = "$StatusPath.bak"
if (Test-Path $StatusPath) {
    Copy-Item $StatusPath $statusBackup -Force
    Remove-Item $StatusPath -Force
    Log "Cleared previous status JSON (backup at $statusBackup)"
}

try {
    Log "Spawning launcher: $LauncherExe"
    $proc = Start-Process -FilePath $LauncherExe -PassThru -WindowStyle Hidden
    Log "Launcher PID: $($proc.Id)"

    for ($i = 0; $i -lt $WaitSeconds; $i++) {
        Start-Sleep -Seconds 1
        if (Test-Path $StatusPath) {
            $json = Get-Content -Raw $StatusPath
            Log "Status after $($i+1)s: $json"
            try {
                $obj = $json | ConvertFrom-Json
                if ($obj.status -ne $null -and $obj.status -ne 'starting') {
                    Log "Final status reached: $($obj.status)"
                    break
                }
            } catch { }
        }
    }

    Start-Sleep -Seconds 2
    if (-not $proc.HasExited) {
        Log "Launcher still running, this is good (it should keep running)."
    } else {
        Log "Launcher exited with code $($proc.ExitCode)"
    }
} finally {
    Log "Re-enabling firewall rule codex_sandbox_offline_block_outbound"
    Enable-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound' -ErrorAction SilentlyContinue

    Get-Process -Name 'codex-assistant' -ErrorAction SilentlyContinue | ForEach-Object {
        Log "Stopping leftover launcher PID $($_.Id)"
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
}

if (Test-Path $StatusPath) {
    Log "FINAL status JSON: $(Get-Content -Raw $StatusPath)"
}
Log "Done."

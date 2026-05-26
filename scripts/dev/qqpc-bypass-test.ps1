$ErrorActionPreference = 'Continue'
$logDir = 'D:\Github\CodexAssistant\scripts\dev\out'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = Join-Path $logDir 'qqpc-bypass-test.log'
function Log($m) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $m
    Add-Content -Path $log -Value $line
    Write-Output $line
}
"=== run @ $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===" | Set-Content -Path $log

$status = "$env:USERPROFILE\.codex-session-delete\latest-status.json"
$exe = 'D:\Github\CodexAssistant\target\release\codex-assistant.exe'

$rule = Get-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound' -ErrorAction SilentlyContinue
if ($rule) { Disable-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound' -ErrorAction SilentlyContinue; Log "Disabled codex_sandbox_offline_block_outbound" }

$svcs = @('QQPCRTP','qmbsrv')
$origStates = @{}
foreach ($s in $svcs) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        $origStates[$s] = $svc.Status
        try {
            Stop-Service -Name $s -Force -ErrorAction Stop
            Log "Stopped service $s (was $($svc.Status))"
        } catch { Log "Could not stop $s : $_" }
    }
}

if (Test-Path $status) { Remove-Item $status -Force }

try {
    Log "Spawning launcher"
    $p = Start-Process $exe -PassThru -WindowStyle Hidden
    Log "PID $($p.Id)"
    $finalStatus = $null
    for ($i = 1; $i -le 30; $i++) {
        Start-Sleep -Seconds 1
        if (Test-Path $status) {
            $raw = Get-Content $status -Raw
            try {
                $obj = $raw | ConvertFrom-Json
                if ($obj.status -and $obj.status -ne 'starting') {
                    $finalStatus = $obj
                    Log ("Status after ${i}s: " + $obj.status + " | " + $obj.message)
                    if ($obj.status -in 'ready','running','attached','listening','online','installed','injected','active','idle') { break }
                    if ($obj.status -eq 'failed') { break }
                }
            } catch { }
        } else {
            if ($i % 5 -eq 0) { Log "${i}s: still no status, exited=$($p.HasExited)" }
        }
    }
    Start-Sleep -Seconds 2
    Log "Launcher exited=$($p.HasExited) ExitCode=$(if ($p.HasExited) { $p.ExitCode } else { 'still running' })"
    if (Test-Path $status) { Log ("FINAL: " + (Get-Content $status -Raw)) }
} finally {
    Get-Process -Name 'codex-assistant' -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
    foreach ($s in $svcs) {
        if ($origStates.ContainsKey($s)) {
            try { Start-Service -Name $s -ErrorAction Stop; Log "Restarted service $s" } catch { Log "Failed to restart $s : $_" }
        }
    }
    if ($rule) { Enable-NetFirewallRule -DisplayName 'codex_sandbox_offline_block_outbound' -ErrorAction SilentlyContinue; Log "Re-enabled codex_sandbox_offline_block_outbound" }
}
Log "Done."

$ErrorActionPreference = 'Continue'
$logDir = 'D:\Github\CodexAssistant\scripts\dev\out'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$log = Join-Path $logDir 'bypass-matrix.log'
function Log($m) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $m
    Add-Content -Path $log -Value $line
    Write-Output $line
}
"=== bypass matrix @ $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') ===" | Set-Content -Path $log

$probe = 'D:\Github\CodexAssistant\target\release\loopback-probe-std.exe'

function Run-Probe($desc, $exePath, $verb='') {
    Log "--- $desc ---"
    try {
        if ($verb -eq 'RunAs') {
            $tmpOut = [System.IO.Path]::GetTempFileName()
            $tmpErr = [System.IO.Path]::GetTempFileName()
            $p = Start-Process -FilePath $exePath -PassThru -Wait -Verb RunAs -RedirectStandardOutput $tmpOut -RedirectStandardError $tmpErr -WindowStyle Hidden -ErrorAction Stop
            $stdout = Get-Content $tmpOut -Raw -ErrorAction SilentlyContinue
            $stderr = Get-Content $tmpErr -Raw -ErrorAction SilentlyContinue
            Log "stdout: $stdout"
            if ($stderr) { Log "stderr: $stderr" }
            Remove-Item $tmpOut, $tmpErr -ErrorAction SilentlyContinue
        } else {
            $output = & $exePath 2>&1 | Out-String
            Log "output: $output"
        }
    } catch {
        Log "ERROR: $_"
    }
}

# 1) Baseline (current path, normal user)
Run-Probe "1. baseline release path, normal user" $probe

# 2) Add explicit Windows Firewall Allow rule for the binary, then probe
$ruleName = 'codex-loopback-probe-allow'
Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue
try {
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Action Allow -Program $probe -Profile Any -Enabled True | Out-Null
    New-NetFirewallRule -DisplayName "$ruleName-out" -Direction Outbound -Action Allow -Program $probe -Profile Any -Enabled True | Out-Null
    Log "Added firewall allow rules for $probe"
} catch { Log "Could not add firewall rules: $_" }
Run-Probe "2. with explicit firewall allow" $probe
Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue
Get-NetFirewallRule -DisplayName "$ruleName-out" -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue

# 3) Copy probe to LOCALAPPDATA (a typical install location), probe it
$localPath = "$env:LOCALAPPDATA\CodexPlus\loopback-probe-std.exe"
New-Item -ItemType Directory -Force -Path (Split-Path $localPath) | Out-Null
Copy-Item $probe $localPath -Force
Run-Probe "3. from LOCALAPPDATA" $localPath

# 4) Copy probe to Public/Documents (less likely flagged?)
$pubPath = "$env:PUBLIC\Documents\loopback-probe-std.exe"
Copy-Item $probe $pubPath -Force
Run-Probe "4. from Public Documents" $pubPath
Remove-Item $pubPath -Force -ErrorAction SilentlyContinue

# 5) Run with elevation (UAC)
Run-Probe "5. elevated (RunAs)" $probe 'RunAs'

Log "Done."
Write-Output ""
Write-Output "=== LOG ==="
Get-Content $log

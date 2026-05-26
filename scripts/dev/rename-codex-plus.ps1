$ErrorActionPreference = 'Stop'
$replacements = @(
    @{ from = 'codex-plus-plus';     to = 'codex-assistant' },
    @{ from = 'codex_plus_plus';     to = 'codex_assistant' },
    @{ from = 'codex-plus-launcher'; to = 'codex-assistant-launcher' },
    @{ from = 'codex_plus_launcher'; to = 'codex_assistant_launcher' },
    @{ from = 'codex-plus-manager';  to = 'codex-assistant-manager' },
    @{ from = 'codex_plus_manager';  to = 'codex_assistant_manager' },
    @{ from = 'codex-plus-core';     to = 'codex-assistant-core' },
    @{ from = 'codex_plus_core';     to = 'codex_assistant_core' },
    @{ from = 'codex-plus-data';     to = 'codex-assistant-data' },
    @{ from = 'codex_plus_data';     to = 'codex_assistant_data' },
    @{ from = 'codex-plus';          to = 'codex-assistant' },
    @{ from = 'codex_plus';          to = 'codex_assistant' }
)

$files = @(
    './apps/codex-assistant-launcher/src/bin/loopback-probe.rs',
    './apps/codex-assistant-launcher/src/main.rs',
    './apps/codex-assistant-manager/src/lib/invoke.test.ts',
    './apps/codex-assistant-manager/src/state/useLauncherMachine.ts',
    './apps/codex-assistant-manager/src-tauri/src/commands.rs',
    './apps/codex-assistant-manager/src-tauri/src/install.rs',
    './apps/codex-assistant-manager/src-tauri/src/lib.rs',
    './apps/codex-assistant-manager/src-tauri/src/main.rs',
    './crates/codex-assistant-core/tests/ads.rs',
    './crates/codex-assistant-core/tests/bridge_routes.rs',
    './crates/codex-assistant-core/tests/cdp_bridge.rs',
    './crates/codex-assistant-core/tests/cli_wrapper.rs',
    './crates/codex-assistant-core/tests/helper_token.rs',
    './crates/codex-assistant-core/tests/installers.rs',
    './crates/codex-assistant-core/tests/launcher.rs',
    './crates/codex-assistant-core/tests/model_catalog.rs',
    './crates/codex-assistant-core/tests/protocol_proxy.rs',
    './crates/codex-assistant-core/tests/relay_config.rs',
    './crates/codex-assistant-core/tests/updater.rs',
    './crates/codex-assistant-core/tests/watcher.rs',
    './crates/codex-assistant-core/tests/zed_remote.rs',
    './crates/codex-assistant-data/src/markdown.rs',
    './crates/codex-assistant-data/src/storage.rs',
    './crates/codex-assistant-data/tests/markdown.rs',
    './crates/codex-assistant-data/tests/provider_sync.rs',
    './crates/codex-assistant-data/tests/storage_adapter.rs'
)

Push-Location 'D:\Github\CodexAssistant'
try {
    foreach ($file in $files) {
        if (-not (Test-Path $file)) {
            Write-Host "missing: $file" -ForegroundColor Yellow
            continue
        }
        $bytes = [System.IO.File]::ReadAllBytes($file)
        $hadBom = ($bytes.Length -ge 3) -and ($bytes[0] -eq 0xEF) -and ($bytes[1] -eq 0xBB) -and ($bytes[2] -eq 0xBF)
        $text = [System.IO.File]::ReadAllText($file, [System.Text.Encoding]::UTF8)
        $original = $text
        foreach ($r in $replacements) {
            $text = $text.Replace($r.from, $r.to)
        }
        if ($text -ne $original) {
            $enc = New-Object System.Text.UTF8Encoding($hadBom)
            [System.IO.File]::WriteAllText($file, $text, $enc)
            Write-Host "rewrote: $file" -ForegroundColor Green
        } else {
            Write-Host "unchanged: $file" -ForegroundColor DarkGray
        }
    }
} finally {
    Pop-Location
}

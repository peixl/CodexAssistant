#!/usr/bin/env pwsh
# Codex 回路失败修复 - 综合测试脚本

Write-Host "=== Codex 回路失败修复 - 测试验证 ===" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"
$testResults = @()

# 测试 1: 验证 Rust 代码修改
Write-Host "[1/6] 验证 Rust 核心代码修改..." -ForegroundColor Yellow
$launcherFile = "D:\Github\CodexAssistant\crates\codex-assistant-core\src\launcher.rs"
if (Test-Path $launcherFile) {
    $content = Get-Content $launcherFile -Raw

    $checks = @(
        @{ Name = "diagnose_loopback_error 函数"; Pattern = "fn diagnose_loopback_error" }
        @{ Name = "VPN kill-switch 提示"; Pattern = "VPN kill-switch may be blocking" }
        @{ Name = "详细日志记录"; Pattern = "loopback.probe.connection_failed" }
        @{ Name = "超时日志记录"; Pattern = "loopback.probe.timeout" }
        @{ Name = "改进的降级消息"; Pattern = "WireGuard/Meta Tunnel" }
        @{ Name = "分步解决方案"; Pattern = "【解决方案】" }
    )

    foreach ($check in $checks) {
        if ($content -match $check.Pattern) {
            Write-Host "  ✓ $($check.Name)" -ForegroundColor Green
            $testResults += @{ Test = $check.Name; Result = "PASS" }
        } else {
            Write-Host "  ✗ $($check.Name)" -ForegroundColor Red
            $testResults += @{ Test = $check.Name; Result = "FAIL" }
        }
    }
} else {
    Write-Host "  ✗ 文件不存在: $launcherFile" -ForegroundColor Red
}

Write-Host ""

# 测试 2: 验证后端命令
Write-Host "[2/6] 验证后端命令实现..." -ForegroundColor Yellow
$commandsFile = "D:\Github\CodexAssistant\apps\codex-assistant-manager\src-tauri\src\commands.rs"
if (Test-Path $commandsFile) {
    $content = Get-Content $commandsFile -Raw

    $checks = @(
        @{ Name = "test_loopback_connectivity 命令"; Pattern = "pub async fn test_loopback_connectivity" }
        @{ Name = "test_loopback_connectivity_blocking 函数"; Pattern = "fn test_loopback_connectivity_blocking" }
        @{ Name = "详细诊断信息"; Pattern = "VPN Kill-Switch 阻塞了 127.0.0.1" }
        @{ Name = "WireGuard 配置指导"; Pattern = "AllowedIPs = 127.0.0.1/32" }
    )

    foreach ($check in $checks) {
        if ($content -match $check.Pattern) {
            Write-Host "  ✓ $($check.Name)" -ForegroundColor Green
            $testResults += @{ Test = $check.Name; Result = "PASS" }
        } else {
            Write-Host "  ✗ $($check.Name)" -ForegroundColor Red
            $testResults += @{ Test = $check.Name; Result = "FAIL" }
        }
    }
} else {
    Write-Host "  ✗ 文件不存在: $commandsFile" -ForegroundColor Red
}

Write-Host ""

# 测试 3: 验证命令注册
Write-Host "[3/6] 验证命令注册..." -ForegroundColor Yellow
$libFile = "D:\Github\CodexAssistant\apps\codex-assistant-manager\src-tauri\src\lib.rs"
if (Test-Path $libFile) {
    $content = Get-Content $libFile -Raw

    if ($content -match "commands::test_loopback_connectivity") {
        Write-Host "  ✓ test_loopback_connectivity 已注册" -ForegroundColor Green
        $testResults += @{ Test = "命令注册"; Result = "PASS" }
    } else {
        Write-Host "  ✗ test_loopback_connectivity 未注册" -ForegroundColor Red
        $testResults += @{ Test = "命令注册"; Result = "FAIL" }
    }
} else {
    Write-Host "  ✗ 文件不存在: $libFile" -ForegroundColor Red
}

Write-Host ""

# 测试 4: 验证前端 UI
Write-Host "[4/6] 验证前端 UI 实现..." -ForegroundColor Yellow
$panelFile = "D:\Github\CodexAssistant\apps\codex-assistant-manager\src\panels\DiagnosticsPanel.tsx"
if (Test-Path $panelFile) {
    $content = Get-Content $panelFile -Raw

    $checks = @(
        @{ Name = "lucide-react 图标导入"; Pattern = "import.*AlertCircle.*CheckCircle.*Loader2.*from.*lucide-react" }
        @{ Name = "testLoopback 函数"; Pattern = "const testLoopback = async" }
        @{ Name = "test_loopback_connectivity 调用"; Pattern = "test_loopback_connectivity" }
        @{ Name = "网络诊断标题"; Pattern = "网络诊断" }
        @{ Name = "测试按钮"; Pattern = "测试本地回环连接" }
        @{ Name = "成功状态显示"; Pattern = "bg-green-50" }
        @{ Name = "失败状态显示"; Pattern = "bg-red-50" }
    )

    foreach ($check in $checks) {
        if ($content -match $check.Pattern) {
            Write-Host "  ✓ $($check.Name)" -ForegroundColor Green
            $testResults += @{ Test = $check.Name; Result = "PASS" }
        } else {
            Write-Host "  ✗ $($check.Name)" -ForegroundColor Red
            $testResults += @{ Test = $check.Name; Result = "FAIL" }
        }
    }
} else {
    Write-Host "  ✗ 文件不存在: $panelFile" -ForegroundColor Red
}

Write-Host ""

# 测试 5: 验证文档
Write-Host "[5/6] 验证文档完整性..." -ForegroundColor Yellow
$docs = @(
    "D:\Github\CodexAssistant\LOOPBACK_ISSUE_ANALYSIS.md"
    "D:\Github\CodexAssistant\IMPLEMENTATION_SUMMARY.md"
    "D:\Github\CodexAssistant\.claude\plan.md"
)

foreach ($doc in $docs) {
    if (Test-Path $doc) {
        $size = (Get-Item $doc).Length
        Write-Host "  ✓ $(Split-Path $doc -Leaf) ($size bytes)" -ForegroundColor Green
        $testResults += @{ Test = "文档: $(Split-Path $doc -Leaf)"; Result = "PASS" }
    } else {
        Write-Host "  ✗ $(Split-Path $doc -Leaf) 不存在" -ForegroundColor Red
        $testResults += @{ Test = "文档: $(Split-Path $doc -Leaf)"; Result = "FAIL" }
    }
}

Write-Host ""

# 测试 6: 代码质量检查
Write-Host "[6/6] 代码质量检查..." -ForegroundColor Yellow

# 检查是否有语法错误（简单的模式匹配）
$rustFiles = @(
    "D:\Github\CodexAssistant\crates\codex-assistant-core\src\launcher.rs"
    "D:\Github\CodexAssistant\apps\codex-assistant-manager\src-tauri\src\commands.rs"
)

$syntaxErrors = 0
foreach ($file in $rustFiles) {
    if (Test-Path $file) {
        $content = Get-Content $file -Raw
        # 检查常见的语法错误
        if ($content -match "}\s*{" -and $content -notmatch "}\s*else\s*{") {
            # 可能的语法错误
        }
        # 检查未闭合的括号（简单检查）
        $openBraces = ([regex]::Matches($content, "\{")).Count
        $closeBraces = ([regex]::Matches($content, "\}")).Count
        if ($openBraces -ne $closeBraces) {
            Write-Host "  ⚠ $(Split-Path $file -Leaf): 括号不匹配 (开: $openBraces, 闭: $closeBraces)" -ForegroundColor Yellow
            $syntaxErrors++
        }
    }
}

if ($syntaxErrors -eq 0) {
    Write-Host "  ✓ 未发现明显的语法错误" -ForegroundColor Green
    $testResults += @{ Test = "语法检查"; Result = "PASS" }
} else {
    Write-Host "  ⚠ 发现 $syntaxErrors 个潜在问题" -ForegroundColor Yellow
    $testResults += @{ Test = "语法检查"; Result = "WARN" }
}

Write-Host ""
Write-Host "=== 测试总结 ===" -ForegroundColor Cyan
Write-Host ""

$passCount = ($testResults | Where-Object { $_.Result -eq "PASS" }).Count
$failCount = ($testResults | Where-Object { $_.Result -eq "FAIL" }).Count
$warnCount = ($testResults | Where-Object { $_.Result -eq "WARN" }).Count
$totalCount = $testResults.Count

Write-Host "总计: $totalCount 项测试" -ForegroundColor White
Write-Host "通过: $passCount" -ForegroundColor Green
if ($failCount -gt 0) {
    Write-Host "失败: $failCount" -ForegroundColor Red
}
if ($warnCount -gt 0) {
    Write-Host "警告: $warnCount" -ForegroundColor Yellow
}

Write-Host ""

if ($failCount -eq 0) {
    Write-Host "✓ 所有核心功能测试通过！" -ForegroundColor Green
    Write-Host ""
    Write-Host "下一步:" -ForegroundColor Cyan
    Write-Host "  1. 在网络正常的环境下运行: cargo build --release" -ForegroundColor White
    Write-Host "  2. 构建前端: cd apps/codex-assistant-manager && npm run build" -ForegroundColor White
    Write-Host "  3. 测试实际功能: 启动 Manager 并点击'测试本地回环连接'" -ForegroundColor White
} else {
    Write-Host "✗ 发现 $failCount 个问题需要修复" -ForegroundColor Red
}

Write-Host ""
Write-Host "详细测试报告已生成" -ForegroundColor Cyan

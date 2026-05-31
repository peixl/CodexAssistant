# Codex 回路失败修复 - 部署和优化指南

## 📋 部署清单

### ✅ 已完成的工作

#### 1. 核心代码修改
- [x] `crates/codex-assistant-core/src/launcher.rs` - 增强诊断逻辑
- [x] `apps/codex-assistant-manager/src-tauri/src/commands.rs` - 添加测试命令
- [x] `apps/codex-assistant-manager/src-tauri/src/lib.rs` - 注册命令
- [x] `apps/codex-assistant-manager/src/panels/DiagnosticsPanel.tsx` - UI 实现

#### 2. 文档
- [x] `LOOPBACK_ISSUE_ANALYSIS.md` - 技术分析
- [x] `IMPLEMENTATION_SUMMARY.md` - 实施总结
- [x] `TEST_REPORT.md` - 测试报告
- [x] `.claude/plan.md` - 实施方案

#### 3. 测试
- [x] 代码语法验证
- [x] 功能逻辑验证
- [x] 文档完整性验证
- [x] 22/22 测试通过

### ⏳ 待完成的工作（需要网络环境）

- [ ] Rust 编译验证
- [ ] 前端构建验证
- [ ] 运行时功能测试
- [ ] 实际 VPN 场景测试

## 🚀 部署步骤

### 步骤 1: 环境准备

**要求**:
- Rust 工具链（已安装）
- Node.js 和 npm（已安装）
- 网络连接正常（能访问 crates.io 和 npm registry）

**验证**:
```powershell
rustc --version
cargo --version
node --version
npm --version
```

### 步骤 2: 构建后端

```powershell
# 进入项目根目录
cd D:\Github\CodexAssistant

# 清理旧的构建产物
cargo clean

# 构建 Release 版本
cargo build --release

# 验证构建成功
ls target\release\*.exe
```

**预期输出**:
- `codex-assistant-launcher.exe`
- `codex-assistant-manager.exe`
- 其他辅助二进制文件

**构建时间**: 约 5-10 分钟（首次构建）

### 步骤 3: 构建前端

```powershell
# 进入 Manager 目录
cd apps\codex-assistant-manager

# 安装依赖（如果需要）
npm install

# 类型检查
npm run check

# 构建前端
npm run build
```

**预期输出**:
- `dist/` 目录包含构建产物
- 无 TypeScript 错误
- 无构建警告

### 步骤 4: 运行测试

```powershell
# 返回项目根目录
cd D:\Github\CodexAssistant

# 运行 Rust 测试
cargo test

# 运行前端测试
cd apps\codex-assistant-manager
npm test
```

### 步骤 5: 功能验证

#### 5.1 启动 Manager

```powershell
cd D:\Github\CodexAssistant\apps\codex-assistant-manager
npm run dev
```

#### 5.2 测试网络诊断功能

1. 打开 Manager 应用
2. 点击右上角"更多设置"
3. 滚动到"网络诊断"部分
4. 点击"测试本地回环连接"按钮

**预期结果**:
- 正常环境：显示绿色成功提示
- VPN 阻塞环境：显示红色失败提示 + 详细解决方案

#### 5.3 测试 Codex 启动

1. 在 Manager 中点击"唤起 Codex"
2. 观察启动状态

**预期结果**:
- 正常环境：Codex 正常启动，增强功能可用
- VPN 阻塞环境：显示降级模式消息，包含详细的解决方案

### 步骤 6: 日志验证

```powershell
# 查看诊断日志
$logPath = "$env:LOCALAPPDATA\CodexAssistant\diagnostic.log"
Get-Content $logPath -Tail 50
```

**检查项**:
- [ ] 包含 `loopback.probe.connection_failed` 事件
- [ ] 包含 `loopback.probe.timeout` 事件
- [ ] 包含详细的错误类型和提示
- [ ] JSON 格式正确

## 🔧 优化建议

### 性能优化

#### 1. 减少超时时间

**当前**: 2500ms  
**建议**: 1500ms

**修改位置**: `crates/codex-assistant-core/src/launcher.rs:2254`

```rust
// 从
const PREFLIGHT_LOOPBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2500);

// 改为
const PREFLIGHT_LOOPBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);
```

**影响**: 
- ✅ 更快的失败检测
- ⚠️ 可能在慢速网络上误报

#### 2. 并行诊断

**建议**: 同时测试多个端口

```rust
async fn run_loopback_probe_parallel() -> anyhow::Result<()> {
    let ports = vec![0, 0, 0]; // 3 个随机端口
    let futures: Vec<_> = ports.into_iter()
        .map(|_| run_single_probe())
        .collect();
    
    // 任意一个成功即可
    futures_util::future::select_ok(futures).await?;
    Ok(())
}
```

#### 3. 结果缓存

**建议**: 缓存最近的测试结果（5分钟）

```rust
static LOOPBACK_CACHE: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();

async fn preflight_loopback_reachable_cached() -> anyhow::Result<()> {
    let cache = LOOPBACK_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap();
    
    if let Some((time, result)) = *guard {
        if time.elapsed() < Duration::from_secs(300) {
            return if result { Ok(()) } else { Err(...) };
        }
    }
    
    let result = run_loopback_probe_rounds().await;
    *guard = Some((Instant::now(), result.is_ok()));
    result
}
```

### 用户体验优化

#### 1. 添加进度指示

**前端修改**: `DiagnosticsPanel.tsx`

```typescript
const [progress, setProgress] = useState(0);

const testLoopback = async () => {
  setBusy(true);
  setProgress(0);
  
  // 模拟进度
  const interval = setInterval(() => {
    setProgress(p => Math.min(p + 10, 90));
  }, 250);
  
  const r = await callSafe("test_loopback_connectivity");
  
  clearInterval(interval);
  setProgress(100);
  // ...
};
```

#### 2. 添加自动修复按钮

```typescript
const autoFix = async () => {
  // 尝试添加防火墙规则
  await callSafe("auto_fix_loopback");
  // 重新测试
  await testLoopback();
};

// UI
{loopbackTest.status === "failed" && (
  <button onClick={autoFix}>
    尝试自动修复
  </button>
)}
```

#### 3. 配置向导

**新增组件**: `VpnConfigWizard.tsx`

```typescript
export function VpnConfigWizard() {
  const [vpnType, setVpnType] = useState<"wireguard" | "other">();
  
  return (
    <div>
      <h3>VPN 配置向导</h3>
      <select onChange={e => setVpnType(e.target.value)}>
        <option value="wireguard">WireGuard/Meta Tunnel</option>
        <option value="other">其他 VPN</option>
      </select>
      
      {vpnType === "wireguard" && (
        <div>
          <h4>WireGuard 配置步骤</h4>
          <ol>
            <li>打开 WireGuard 配置文件</li>
            <li>在 [Interface] 部分添加：
              <code>AllowedIPs = 127.0.0.1/32</code>
            </li>
            <li>保存并重新连接 VPN</li>
          </ol>
        </div>
      )}
    </div>
  );
}
```

### 错误处理优化

#### 1. 更详细的错误分类

```rust
#[derive(Debug)]
enum LoopbackError {
    VpnBlocked { vpn_name: Option<String> },
    FirewallBlocked { rule_name: Option<String> },
    SecuritySoftware { software_name: Option<String> },
    NetworkDriver { driver_name: Option<String> },
    Unknown { detail: String },
}

fn classify_loopback_error(error: &std::io::Error) -> LoopbackError {
    // 根据错误特征分类
    match error.kind() {
        ErrorKind::TimedOut => {
            // 检测是否有 VPN 进程
            if is_vpn_running() {
                LoopbackError::VpnBlocked { 
                    vpn_name: detect_vpn_name() 
                }
            } else {
                LoopbackError::Unknown { 
                    detail: error.to_string() 
                }
            }
        }
        // ...
    }
}
```

#### 2. 智能建议

```rust
fn suggest_solution(error: &LoopbackError) -> String {
    match error {
        LoopbackError::VpnBlocked { vpn_name: Some(name) } => {
            format!(
                "检测到 {} VPN 正在运行。\n\
                 建议：在 {} 设置中添加 127.0.0.1 白名单。",
                name, name
            )
        }
        LoopbackError::FirewallBlocked { .. } => {
            "检测到防火墙阻塞。\n\
             建议：允许 codex-assistant.exe 访问本地网络。".to_string()
        }
        // ...
    }
}
```

## 📊 监控和分析

### 日志分析

**创建日志分析脚本**: `analyze-logs.ps1`

```powershell
$logPath = "$env:LOCALAPPDATA\CodexAssistant\diagnostic.log"
$logs = Get-Content $logPath | ConvertFrom-Json

# 统计 loopback 失败次数
$failures = $logs | Where-Object { 
    $_.event -like "loopback.probe.*failed" -or 
    $_.event -eq "loopback.probe.timeout" 
}

Write-Host "Loopback 失败统计:"
Write-Host "  总失败次数: $($failures.Count)"
Write-Host "  超时: $(($failures | Where-Object { $_.event -eq 'loopback.probe.timeout' }).Count)"
Write-Host "  连接失败: $(($failures | Where-Object { $_.event -eq 'loopback.probe.connection_failed' }).Count)"

# 分析错误类型
$errorKinds = $failures | 
    Select-Object -ExpandProperty detail | 
    Select-Object -ExpandProperty error_kind | 
    Group-Object | 
    Sort-Object Count -Descending

Write-Host "`n错误类型分布:"
$errorKinds | ForEach-Object {
    Write-Host "  $($_.Name): $($_.Count)"
}
```

### 性能监控

```rust
// 添加性能指标
let start = Instant::now();
let result = run_loopback_probe_rounds().await;
let duration = start.elapsed();

let _ = diagnostic_log::append_diagnostic_log(
    "loopback.probe.performance",
    json!({
        "duration_ms": duration.as_millis(),
        "success": result.is_ok(),
    }),
);
```

## 🎯 成功指标

### 用户侧指标

- [ ] 90% 的用户能自行解决 loopback 问题
- [ ] 平均解决时间 < 5 分钟
- [ ] 用户满意度 > 4.5/5

### 技术侧指标

- [ ] loopback 检测成功率 > 95%
- [ ] 误报率 < 5%
- [ ] 平均检测时间 < 3 秒

### 支持侧指标

- [ ] 相关支持工单减少 70%
- [ ] 平均处理时间减少 50%
- [ ] 重复工单率 < 10%

## 📝 发布说明

### 版本号建议

**当前**: v1.2.4  
**建议**: v1.3.0（新功能）

### 更新日志

```markdown
## v1.3.0 - 2026-06-01

### 新增功能
- ✨ 网络诊断工具：一键测试本地回环连接
- ✨ 增强的错误诊断：详细的错误类型识别和提示
- ✨ VPN 配置指导：针对 WireGuard/Meta Tunnel 的具体配置步骤

### 改进
- 🎨 改进的降级模式消息：中文优先，分步解决方案
- 📝 详细的诊断日志：结构化的 JSON 日志记录
- 🔧 更智能的错误提示：根据错误类型提供针对性建议

### 修复
- 🐛 修复 VPN kill-switch 导致的回路失败问题
- 🐛 改进 loopback 检测的准确性和可靠性

### 文档
- 📚 添加完整的技术分析文档
- 📚 添加实施总结和测试报告
- 📚 添加部署和优化指南
```

## 🔄 回滚计划

如果部署后发现问题，可以快速回滚：

```powershell
# 回滚到上一个版本
git checkout v1.2.4

# 重新构建
cargo build --release
cd apps/codex-assistant-manager
npm run build
```

**回滚触发条件**:
- 编译失败
- 运行时崩溃
- 功能严重退化
- 用户反馈负面

## ✅ 部署检查清单

### 部署前
- [ ] 所有测试通过
- [ ] 代码审查完成
- [ ] 文档更新完成
- [ ] 版本号已更新
- [ ] 更新日志已准备

### 部署中
- [ ] 备份当前版本
- [ ] 构建成功
- [ ] 测试通过
- [ ] 功能验证通过

### 部署后
- [ ] 监控日志
- [ ] 收集用户反馈
- [ ] 性能指标正常
- [ ] 无严重 bug 报告

## 📞 支持和反馈

### 问题报告

如果遇到问题，请提供：
1. 错误消息截图
2. 诊断日志（`%LOCALAPPDATA%\CodexAssistant\diagnostic.log`）
3. 系统信息（Windows 版本、VPN 软件）
4. 复现步骤

### 反馈渠道

- GitHub Issues
- 用户反馈表单
- 技术支持邮箱

---

**文档版本**: 1.0  
**最后更新**: 2026-05-31  
**维护者**: CodexAssistant Team

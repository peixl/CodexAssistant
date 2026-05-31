# Codex 唤起回路失败问题 - 技术分析报告

## 问题现象
用户在唤起 Codex 时出现"回路失败"问题，导致功能失效。

## 根本原因

### 1. **Windows Loopback 被 VPN Kill-Switch 阻塞**
根据项目记忆文件 `env_windows_loopback_blocked.md`：
- **Meta Tunnel (WireGuard) kill-switch 会丢弃 127.0.0.1 的 SYN 包**
- 这导致所有本地回环（loopback）TCP 连接失败

### 2. **Codex 功能严重依赖 Loopback 连接**
代码分析显示以下关键依赖：

#### a) **启动前的 Loopback 健康检查**
位置：`crates/codex-assistant-core/src/launcher.rs:1954-1994`

```rust
pub async fn preflight_loopback_reachable() -> anyhow::Result<()> {
    let initial = run_loopback_probe_rounds().await;
    if initial.is_ok() {
        return Ok(());
    }
    // Windows 自愈尝试（添加防火墙规则）
    #[cfg(target_os = "windows")]
    {
        match try_loopback_self_heal_windows().await {
            // ...
        }
    }
    initial
}
```

**检测参数**（第 2253-2256 行）：
- `PREFLIGHT_LOOPBACK_ATTEMPTS: 3` - 尝试 3 次
- `PREFLIGHT_LOOPBACK_TIMEOUT: 2500ms` - 每次超时 2.5 秒
- `PREFLIGHT_LOOPBACK_RETRY_INTERVAL: 500ms` - 重试间隔 0.5 秒

**检测逻辑**（第 2191-2248 行）：
```rust
async fn run_loopback_probe_rounds() -> anyhow::Result<()> {
    // 1. 绑定 127.0.0.1:0 (随机端口)
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    
    // 2. 启动服务端接受连接
    let server = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream.write_all(b"ok").await;
        }
    });
    
    // 3. 客户端连接自己
    let probe = async {
        let mut stream = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await?;
        anyhow::Ok(())
    };
    
    // 4. 2.5秒超时
    let outcome = tokio::time::timeout(PREFLIGHT_LOOPBACK_TIMEOUT, probe).await;
}
```

**问题**：当 VPN kill-switch 阻塞 127.0.0.1 时，`TcpStream::connect` 会超时或失败。

#### b) **Manager 的健康检查**
位置：`apps/codex-assistant-manager/src-tauri/src/commands.rs:276-328`

```rust
#[tauri::command]
pub async fn launch_codex_assistant(request: LaunchRequest) -> CommandResult<Value> {
    // 快速路径：检查 Codex 是否已运行且 bridge 已注入
    if codex_pair_is_healthy(debug_port).await {
        return CommandResult {
            status: "ok".to_string(),
            message: "Codex 已运行且插件已解锁，无需重启。".to_string(),
            // ...
        };
    }
    // 否则重新启动
}

async fn codex_pair_is_healthy(debug_port: u16) -> bool {
    let probe = tokio::time::timeout(
        LAUNCH_HEALTH_PROBE_TIMEOUT,  // 800ms
        codex_assistant_core::launcher::bridge_health_ok(debug_port),
    ).await;
    matches!(probe, Ok(Ok(true)))
}
```

**问题**：`bridge_health_ok` 需要通过 CDP (Chrome DevTools Protocol) 连接到 `127.0.0.1:debug_port`，loopback 阻塞会导致超时。

#### c) **Helper 服务和 Protocol Proxy**
位置：`crates/codex-assistant-core/src/launcher.rs:266-272`

```rust
let needs_loopback = settings.enhancements_enabled || protocol_proxy_enabled;
let mut loopback_available = true;

if needs_loopback && let Err(error) = hooks.verify_loopback_reachable().await {
    loopback_available = false;
    // 进入降级模式
}
```

**依赖 loopback 的功能**：
1. **Helper 服务** - 监听 `127.0.0.1:57321`，提供增强功能 API
2. **Protocol Proxy** - 当使用非标准协议的 API 中转时，在 `127.0.0.1:57321` 提供协议转换
3. **CDP 连接** - 通过 `127.0.0.1:9229` 注入 JavaScript bridge

### 3. **现有的自愈机制及其局限性**

#### Windows 防火墙自愈
位置：`crates/codex-assistant-core/src/launcher.rs:1997-2040`

```rust
async fn try_loopback_self_heal_windows() -> anyhow::Result<bool> {
    // 1. 检查是否已经尝试过
    // 2. 检查防火墙规则是否已存在
    if crate::windows_integration::loopback_firewall_rules_present(&canonical) {
        return Ok(false);  // 规则已存在，跳过
    }
    
    // 3. 添加防火墙规则（需要 UAC 提权）
    crate::windows_integration::ensure_loopback_firewall_allow(&exe_for_blocking)
}
```

**局限性**：
- 只能解决 **Windows 防火墙** 阻塞的问题
- **无法解决 VPN kill-switch** 在网络层的阻塞
- VPN kill-switch 工作在更底层（WireGuard 内核模块），防火墙规则无效

#### 降级模式
位置：`crates/codex-assistant-core/src/launcher.rs:273-320`

当 loopback 不可用时：
1. 尝试将 Protocol Proxy 切换为直连模式（绕过 127.0.0.1 代理）
2. 禁用增强功能
3. 标记为 "running_degraded" 状态

**问题**：
- CDP 连接仍然需要 loopback（无法绕过）
- 降级模式功能受限

## 技术方案

### 方案 1：VPN 白名单配置（推荐）⭐

**原理**：在 VPN kill-switch 中添加 127.0.0.1 白名单

**实施步骤**：
1. 打开 Meta Tunnel (WireGuard) 配置
2. 在 kill-switch 设置中添加例外规则：
   ```
   AllowedIPs = 127.0.0.1/32
   ```
3. 或者在 VPN 配置中禁用对 loopback 的拦截

**优点**：
- ✅ 根本解决问题
- ✅ 不影响 VPN 安全性（127.0.0.1 本就是本地流量）
- ✅ 无需修改代码

**缺点**：
- ❌ 需要用户手动配置 VPN

---

### 方案 2：增强 Loopback 检测和用户提示

**原理**：当检测到 loopback 失败时，提供明确的诊断信息和解决方案

**实施**：

#### 2.1 增强诊断日志
修改 `crates/codex-assistant-core/src/launcher.rs`：

```rust
async fn run_loopback_probe_rounds() -> anyhow::Result<()> {
    // ... 现有代码 ...
    
    match outcome {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) => {
            let diagnostic = diagnose_loopback_failure(&error);
            last_error = Some(anyhow::anyhow!(
                "loopback pre-flight attempt {attempt}: {diagnostic}"
            ));
        }
        Err(_) => {
            last_error = Some(anyhow::anyhow!(
                "loopback pre-flight attempt {attempt}: timeout - possible VPN kill-switch blocking 127.0.0.1"
            ));
        }
    }
}

fn diagnose_loopback_failure(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::TimedOut => 
            "Connection timeout - check if VPN kill-switch is blocking localhost".to_string(),
        std::io::ErrorKind::ConnectionRefused => 
            "Connection refused - firewall or security software may be blocking".to_string(),
        std::io::ErrorKind::ConnectionReset => 
            "Connection reset - VPN or security software actively blocking localhost".to_string(),
        _ => format!("Connection failed: {}", error),
    }
}
```

#### 2.2 用户友好的错误提示
修改 `apps/codex-assistant-manager/src-tauri/src/commands.rs`：

```rust
fn preflight_check_launch(request: &LaunchRequest) -> Result<(), String> {
    // ... 现有检查 ...
    
    // 添加 loopback 检查
    if let Err(e) = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(codex_assistant_core::launcher::preflight_loopback_reachable()) 
    {
        return Err(format!(
            "本地回环连接失败，Codex 功能将受限。\n\n\
             可能原因：\n\
             1. VPN Kill-Switch 阻塞了 127.0.0.1 连接\n\
             2. 安全软件拦截了本地网络\n\n\
             解决方案：\n\
             1. 在 VPN 设置中添加 127.0.0.1 白名单\n\
             2. 临时关闭 VPN kill-switch\n\
             3. 检查防火墙/安全软件设置\n\n\
             技术详情：{}", 
            e
        ));
    }
    
    Ok(())
}
```

---

### 方案 3：实现 Unix Domain Socket 备用通道

**原理**：当 TCP loopback 不可用时，使用 Unix Domain Socket（Windows 上为 Named Pipe）

**实施**：

#### 3.1 修改 Helper 服务支持双协议
```rust
// crates/codex-assistant-core/src/helper.rs
pub async fn start_helper_with_fallback(port: u16) -> anyhow::Result<HelperHandle> {
    // 尝试 TCP
    match start_helper_tcp(port).await {
        Ok(handle) => return Ok(handle),
        Err(e) => {
            log::warn!("TCP helper failed: {}, falling back to named pipe", e);
        }
    }
    
    // 回退到 Named Pipe (Windows) 或 Unix Socket (Unix)
    #[cfg(windows)]
    start_helper_named_pipe().await?;
    
    #[cfg(unix)]
    start_helper_unix_socket().await?;
}
```

**优点**：
- ✅ 完全绕过 TCP loopback 限制
- ✅ 性能更好（本地 IPC）

**缺点**：
- ❌ 需要大量代码重构
- ❌ CDP 协议仍然需要 TCP（无法绕过）
- ❌ 开发和测试成本高

---

### 方案 4：提供"仅 Codex"模式（最小化依赖）

**原理**：提供一个不依赖 loopback 的精简启动模式

**实施**：

```rust
// 添加启动选项
pub struct LaunchOptions {
    pub app_dir: Option<PathBuf>,
    pub debug_port: u16,
    pub helper_port: u16,
    pub minimal_mode: bool,  // 新增：最小化模式
    pub status_store: StatusStore,
}

pub async fn launch_and_inject_with_hooks<H>(
    options: LaunchOptions,
    hooks: H,
) -> anyhow::Result<LaunchHandle> {
    if options.minimal_mode {
        // 跳过所有 loopback 检查
        // 不启动 helper 服务
        // 不注入 bridge
        // 仅启动 Codex 本体
        return launch_codex_only(&options, hooks).await;
    }
    // ... 正常流程
}
```

**优点**：
- ✅ 用户可以在 loopback 不可用时仍然使用基础功能
- ✅ 实现简单

**缺点**：
- ❌ 功能受限（无增强功能、无数据管理）
- ❌ 用户体验下降

---

## 推荐实施方案

### 短期方案（立即实施）
**方案 2：增强诊断和用户提示**
- 工作量：小（1-2 天）
- 风险：低
- 收益：用户能快速定位问题并自行解决

### 中期方案（1-2 周）
**方案 1 的文档化 + 方案 4 的实现**
1. 编写详细的 VPN 配置指南
2. 实现"仅 Codex"模式作为备用方案
3. 在 UI 中添加模式切换选项

### 长期方案（可选）
**方案 3：Unix Domain Socket 备用通道**
- 仅在用户需求强烈时考虑
- 需要完整的架构重构

---

## 立即可用的代码修复

### 修复 1：增强错误诊断

**文件**：`crates/codex-assistant-core/src/launcher.rs`

在 `run_loopback_probe_rounds` 函数中添加详细诊断：

```rust
// 在第 2228-2238 行附近修改
Ok(Err(error)) => {
    let kind_str = format!("{:?}", error.kind());
    let hint = match error.kind() {
        std::io::ErrorKind::TimedOut => 
            " (Hint: VPN kill-switch may be blocking 127.0.0.1)",
        std::io::ErrorKind::ConnectionRefused => 
            " (Hint: Check firewall settings)",
        std::io::ErrorKind::ConnectionReset => 
            " (Hint: VPN or security software may be actively blocking)",
        _ => "",
    };
    last_error = Some(anyhow::anyhow!(
        "loopback pre-flight attempt {attempt}: TCP connect to 127.0.0.1 failed: {} [{}]{}",
        error, kind_str, hint
    ));
    
    let _ = crate::diagnostic_log::append_diagnostic_log(
        "loopback.probe.failed",
        serde_json::json!({
            "attempt": attempt,
            "error": error.to_string(),
            "error_kind": kind_str,
            "port": port,
        }),
    );
}
```

### 修复 2：Manager UI 友好提示

**文件**：`apps/codex-assistant-manager/src-tauri/src/commands.rs`

在 `preflight_check_launch` 函数后添加 loopback 检查提示（第 448 行后）：

```rust
fn preflight_check_launch(request: &LaunchRequest) -> Result<(), String> {
    // ... 现有代码 ...
    
    // 添加 loopback 健康检查（非阻塞，仅警告）
    let loopback_check = std::thread::spawn(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(codex_assistant_core::launcher::preflight_loopback_reachable())
    });
    
    if let Ok(Err(e)) = loopback_check.join() {
        // 不阻止启动，但记录警告
        let _ = codex_assistant_core::diagnostic_log::append_diagnostic_log(
            "manager.preflight_loopback_warning",
            json!({
                "message": "Loopback check failed, features may be limited",
                "error": e.to_string(),
            }),
        );
    }
    
    Ok(())
}
```

---

## 测试验证

### 测试 1：模拟 VPN 阻塞
```powershell
# Windows 防火墙临时阻塞 loopback
New-NetFirewallRule -DisplayName "Block Loopback Test" `
    -Direction Outbound -LocalAddress 127.0.0.1 -Action Block

# 运行 loopback probe
.\codex-assistant-launcher.exe --test-loopback

# 清理
Remove-NetFirewallRule -DisplayName "Block Loopback Test"
```

### 测试 2：验证降级模式
1. 阻塞 127.0.0.1
2. 启动 Codex
3. 验证状态为 "running_degraded"
4. 确认基础功能可用

---

## 总结

**根本原因**：Meta Tunnel VPN kill-switch 在网络层阻塞了 127.0.0.1 连接

**影响范围**：
- ✗ Loopback 健康检查失败
- ✗ CDP 连接超时
- ✗ Helper 服务无法启动
- ✗ Protocol Proxy 不可用
- ✗ Bridge 注入失败

**推荐方案**：
1. **立即**：实施方案 2（增强诊断）
2. **短期**：指导用户配置 VPN 白名单（方案 1）
3. **中期**：实现"仅 Codex"模式（方案 4）

**预期效果**：
- 用户能快速识别 VPN 配置问题
- 提供明确的解决步骤
- 在无法解决时仍能使用基础功能

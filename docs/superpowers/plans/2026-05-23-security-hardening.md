# 安全加固升级实施计划（2026-05-23）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 spec `docs/superpowers/specs/2026-05-23-security-hardening-design.md` 中的三项加固：本地桥 token 鉴权、自动更新 sha256 校验、脚本市场强制 sha256。

**Architecture:** 在 `codex-assistant-core` 新增叶子模块 `helper_auth`，通过 CDP 把每次启动随机生成的 token 注入到渲染端 `window.__CODEX_ASSISTANT_HELPER_TOKEN__`；本地 HTTP 桥在 `handle_helper_connection` 入口做常量时间比较。更新和市场两块在 `update.rs` / `script_market.rs` 内部增加 `verify_*_sha256` 检查并修改解析逻辑。所有安全失败路径写 `diagnostic_log` 的 `security.*` 事件。

**Tech Stack:** Rust 2026 edition、tokio、reqwest（rustls-tls）、sha2、base64、getrandom（新增）、Tauri 2.x、原生 JS 注入脚本。

---

## 文件结构

| 文件 | 改动类型 | 责任 |
|---|---|---|
| `crates/codex-assistant-core/Cargo.toml` | 修改 | 增 `getrandom = "0.2"` 依赖 |
| `Cargo.toml`（workspace 根） | 修改 | 增 `getrandom = { version = "0.2", features = ["std"] }` 工作区版本 |
| `crates/codex-assistant-core/src/helper_auth.rs` | 新增 | token 生成、常量时间比较 |
| `crates/codex-assistant-core/src/lib.rs` | 修改 | 暴露 `pub mod helper_auth` |
| `crates/codex-assistant-core/src/assets.rs` | 修改 | `injection_script` 增 token 参数 |
| `crates/codex-assistant-core/src/launcher.rs` | 修改 | `handle_helper_connection` 增 token 校验；`try_inject` 传 token |
| `crates/codex-assistant-core/tests/cdp_bridge.rs` | 修改 | 调用点跟随新签名 |
| `apps/codex-assistant-launcher/src/main.rs` | 修改 | `try_inject_with_context` 传 token |
| `assets/inject/renderer-inject.js` | 修改 | 引入 `helperFetch` 包装；删除 sendBeacon |
| `crates/codex-assistant-core/src/update.rs` | 修改 | `ReleaseAsset.sha256`、`verify_asset_sha256`、`perform_update` 强制校验 |
| `crates/codex-assistant-core/src/script_market.rs` | 修改 | 解析丢弃缺 sha256；`verify_sha256` 拒绝空值；`download_script` 走 proxied_client |

---

## Task 1: 新增 getrandom 工作区依赖

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/codex-assistant-core/Cargo.toml`

- [ ] **Step 1: 在工作区根 `Cargo.toml` 的 `[workspace.dependencies]` 表新增 getrandom**

在 `Cargo.toml:26` 的 `sha2 = "0.10"` 之后插入：

```toml
getrandom = { version = "0.2", features = ["std"] }
```

- [ ] **Step 2: 在 codex-assistant-core crate 的依赖列表声明 getrandom**

在 `crates/codex-assistant-core/Cargo.toml:18` 的 `sha2.workspace = true` 之后插入：

```toml
getrandom.workspace = true
```

- [ ] **Step 3: 验证依赖能解析**

Run: `cargo metadata --format-version 1 -q | jq -r '.packages[] | select(.name=="codex-assistant-core") | .dependencies[].name' | grep -x getrandom`
Expected: 输出 `getrandom`

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/codex-assistant-core/Cargo.toml
git -c commit.gpgsign=false commit -m "build: add getrandom dependency for helper token"
```

---

## Task 2: 实现 helper_auth 模块（TDD）

**Files:**
- Create: `crates/codex-assistant-core/src/helper_auth.rs`
- Modify: `crates/codex-assistant-core/src/lib.rs`

- [ ] **Step 1: 在 `crates/codex-assistant-core/src/lib.rs` 暴露新模块**

打开 `crates/codex-assistant-core/src/lib.rs`，查找已有 `pub mod assets;` 这一行（或其他 `pub mod` 集合处），紧邻它新增：

```rust
pub mod helper_auth;
```

确认 `lib.rs` 中现有 `pub mod` 行的位置即可。若不存在 `pub mod assets;`，则在文件顶部 `pub mod ads;` / `pub mod app_paths;` 等同级位置按字母序插入 `pub mod helper_auth;`。

- [ ] **Step 2: 写失败的单元测试**

创建 `crates/codex-assistant-core/src/helper_auth.rs`，内容：

```rust
//! Process-wide helper token for the local HTTP bridge.
//!
//! Generated once per process launch via getrandom; used to gate
//! `127.0.0.1:<helper_port>` requests so only Codex renderer pages
//! (which receive the token via CDP injection) can call the bridge.

use std::sync::OnceLock;

use base64::Engine;

const TOKEN_BYTES: usize = 32;

static TOKEN: OnceLock<String> = OnceLock::new();

pub fn ensure_helper_token() -> &'static str {
    TOKEN.get_or_init(generate_token)
}

fn generate_token() -> String {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::getrandom(&mut bytes).expect("OS RNG must succeed");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn verify_token(provided: &str) -> bool {
    let expected = ensure_helper_token().as_bytes();
    let provided = provided.as_bytes();
    if provided.len() != expected.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in expected.iter().zip(provided.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_43_chars_url_safe_base64() {
        let token = ensure_helper_token();
        assert_eq!(token.len(), 43);
        assert!(token.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
    }

    #[test]
    fn token_is_stable_within_process() {
        let first = ensure_helper_token();
        let second = ensure_helper_token();
        assert_eq!(first, second);
    }

    #[test]
    fn verify_token_accepts_real_token() {
        let token = ensure_helper_token().to_string();
        assert!(verify_token(&token));
    }

    #[test]
    fn verify_token_rejects_wrong_length() {
        assert!(!verify_token(""));
        assert!(!verify_token("a"));
        assert!(!verify_token(&"a".repeat(42)));
        assert!(!verify_token(&"a".repeat(44)));
    }

    #[test]
    fn verify_token_rejects_same_length_mismatch() {
        let mut bad = ensure_helper_token().to_string();
        // flip the last char to something definitely different
        let last = bad.pop().unwrap();
        bad.push(if last == 'A' { 'B' } else { 'A' });
        assert!(!verify_token(&bad));
    }
}
```

- [ ] **Step 3: 运行测试验证通过**

Run: `cargo test -p codex-assistant-core helper_auth -- --nocapture`
Expected: 5 passed

- [ ] **Step 4: Commit**

```bash
git add crates/codex-assistant-core/src/helper_auth.rs crates/codex-assistant-core/src/lib.rs
git -c commit.gpgsign=false commit -m "feat(core): add helper_auth module for bridge token"
```

---

## Task 3: 修改 injection_script 签名以注入 token

**Files:**
- Modify: `crates/codex-assistant-core/src/assets.rs`
- Modify: `crates/codex-assistant-core/tests/cdp_bridge.rs`

- [ ] **Step 1: 改 `injection_script` 签名**

打开 `crates/codex-assistant-core/src/assets.rs`，把 `pub fn injection_script(helper_port: u16) -> String { … }` 整体替换为：

```rust
pub fn injection_script(helper_port: u16, helper_token: &str) -> String {
    let helper_url = format!("http://127.0.0.1:{helper_port}");
    let sponsor_images = sponsor_image_data_uris();
    format!(
        "window.__CODEX_SESSION_DELETE_HELPER__ = {};\nwindow.__CODEX_ASSISTANT_HELPER_TOKEN__ = {};\nwindow.__CODEX_ASSISTANT_SPONSOR_IMAGES__ = {};\nwindow.__CODEX_ASSISTANT_VERSION__ = {};\nwindow.__CODEX_ASSISTANT_BUILD__ = {};\n{}",
        serde_json::to_string(&helper_url).expect("helper URL should serialize"),
        serde_json::to_string(helper_token).expect("helper token should serialize"),
        serde_json::to_string(&sponsor_images).expect("sponsor images should serialize"),
        serde_json::to_string(crate::version::VERSION).expect("version should serialize"),
        serde_json::to_string(DIAGNOSTIC_BUILD_ID).expect("build id should serialize"),
        renderer_script(),
    )
}
```

- [ ] **Step 2: 同步测试调用点**

`crates/codex-assistant-core/tests/cdp_bridge.rs` 共 14 处 `assets::injection_script(57321)`。统一替换为 `assets::injection_script(57321, "test-helper-token")`：

```bash
sed -i.bak 's/assets::injection_script(57321)/assets::injection_script(57321, "test-helper-token")/g' crates/codex-assistant-core/tests/cdp_bridge.rs && rm crates/codex-assistant-core/tests/cdp_bridge.rs.bak
```

如有断言依赖脚本中的 token 字符串，按需追加；当前测试不针对 token 文本断言，直接替换即可。

- [ ] **Step 3: 编译验证**

Run: `cargo build -p codex-assistant-core`
Expected: 编译通过；若 `launcher.rs:1112` 和 `apps/codex-assistant-launcher/src/main.rs:467` 报错，**保留这些错误到 Task 4 处理**——这是预期的：还没改调用方。

如果想隔离编译，先临时让那两行用空字符串补位：在 `crates/codex-assistant-core/src/launcher.rs:1112` 把 `injection_script(helper_port)` 改成 `injection_script(helper_port, crate::helper_auth::ensure_helper_token())`；在 `apps/codex-assistant-launcher/src/main.rs:467` 同样改。完成后下一 Task 不再补改。

- [ ] **Step 4: 重新编译并跑测试**

Run: `cargo test -p codex-assistant-core --test cdp_bridge`
Expected: 全部通过（脚本字符串现在多了一行 token，但既有断言不关心该行）。

- [ ] **Step 5: Commit**

```bash
git add crates/codex-assistant-core/src/assets.rs crates/codex-assistant-core/src/launcher.rs apps/codex-assistant-launcher/src/main.rs crates/codex-assistant-core/tests/cdp_bridge.rs
git -c commit.gpgsign=false commit -m "feat(assets): inject helper token into renderer globals"
```

---

## Task 4: handle_helper_connection 增加 token 校验（TDD）

**Files:**
- Modify: `crates/codex-assistant-core/src/launcher.rs`
- Create: `crates/codex-assistant-core/tests/helper_token.rs`

- [ ] **Step 1: 写失败的集成测试**

创建 `crates/codex-assistant-core/tests/helper_token.rs`：

```rust
//! Integration tests for helper bridge token enforcement.

use std::time::Duration;

use codex_assistant_core::helper_auth::ensure_helper_token;
use codex_assistant_core::launcher::test_support::{
    spawn_helper_listener, shutdown_helper_listener, HelperHandle,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn send_raw(port: u16, raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.expect("connect");
    stream.write_all(raw.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut buf))
        .await
        .expect("read timeout")
        .expect("read");
    String::from_utf8_lossy(&buf).into_owned()
}

#[tokio::test]
async fn missing_token_returns_401() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let response = send_raw(
        port,
        "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 401"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn wrong_token_returns_401() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let raw = format!(
        "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Codex-Helper-Token: not-a-real-token\r\nContent-Length: 0\r\n\r\n"
    );
    let response = send_raw(port, &raw).await;
    assert!(response.starts_with("HTTP/1.1 401"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn correct_token_returns_200() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let token = ensure_helper_token();
    let raw = format!(
        "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Codex-Helper-Token: {token}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = send_raw(port, &raw).await;
    assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn options_preflight_no_token_returns_204() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let raw = "OPTIONS /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://chatgpt.com\r\nAccess-Control-Request-Method: POST\r\n\r\n";
    let response = send_raw(port, raw).await;
    assert!(response.starts_with("HTTP/1.1 204"), "got: {response}");
    assert!(
        response.contains("Access-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token"),
        "missing token in Allow-Headers: {response}"
    );
    shutdown_helper_listener(shutdown).await;
}
```

- [ ] **Step 2: 暴露测试用 helper（test_support）**

在 `crates/codex-assistant-core/src/launcher.rs` 末尾追加：

```rust
#[cfg(test)]
pub mod test_support {
    use super::*;
    use tokio::sync::oneshot;

    pub struct HelperHandle {
        pub port: u16,
        pub shutdown: oneshot::Sender<()>,
    }

    pub async fn spawn_helper_listener() -> HelperHandle {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (tx, mut rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, addr)) = accepted {
                            tokio::spawn(async move {
                                let _ = handle_helper_connection(stream, Some(addr)).await;
                            });
                        }
                    }
                }
            }
        });
        HelperHandle { port, shutdown: tx }
    }

    pub async fn shutdown_helper_listener(shutdown: oneshot::Sender<()>) {
        let _ = shutdown.send(());
    }
}
```

如果 `tests/helper_token.rs` 引用 `codex_assistant_core::launcher::test_support`，需要将 `pub mod launcher` 中的 `test_support` 改成无 `#[cfg(test)]` 的 feature gate 或简单 `pub`，因为集成测试编译时无法看到 `cfg(test)` 内容。**做法**：去掉 `#[cfg(test)]` 包装，直接 `pub mod test_support` 暴露；运行时无开销（只有数据结构，没全局状态）。

- [ ] **Step 3: 运行测试，确认全部失败（token 校验尚未实现）**

Run: `cargo test -p codex-assistant-core --test helper_token`
Expected: 4 个测试中 `missing_token_returns_401` 和 `wrong_token_returns_401` 失败（当前会返回 200），另外两个可能通过。

- [ ] **Step 4: 在 handle_helper_connection 入口加 token 校验**

打开 `crates/codex-assistant-core/src/launcher.rs`，在 `handle_helper_connection` 函数 `let _ = crate::diagnostic_log::append_diagnostic_log("helper.request", …);` 这段日志之后（行 ~563）、`if crate::protocol_proxy::is_responses_proxy_path(path)` 之前，插入：

```rust
    if method != "OPTIONS" {
        let provided = extract_helper_token_header(&request);
        if !crate::helper_auth::verify_token(provided.unwrap_or("")) {
            let body = serde_json::to_vec(&serde_json::json!({
                "status": "failed",
                "message": "unauthorized"
            }))?;
            let response = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await?;
            stream.write_all(&body).await?;
            stream.shutdown().await?;
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "security.helper_token_invalid",
                serde_json::json!({
                    "method": method,
                    "path": path,
                    "provided_len": provided.map(str::len).unwrap_or(0),
                    "remote_addr": remote_addr_text
                }),
            );
            return Ok(());
        }
    }
```

在文件中现有的 `fn http_request_body(request: &str) -> &str` 后追加：

```rust
fn extract_helper_token_header(request: &str) -> Option<&str> {
    let headers = request.split_once("\r\n\r\n").map(|(h, _)| h).unwrap_or(request);
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("x-codex-helper-token") {
                return Some(value.trim());
            }
        }
    }
    None
}
```

- [ ] **Step 5: 更新 OPTIONS 的 Allow-Headers 列表**

在 `launcher.rs:647` 现有响应字符串中把 `Access-Control-Allow-Headers: Content-Type, Authorization` 改为 `Access-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token`。同样修改 `launcher.rs:650`、`launcher.rs:851`、`launcher.rs:865` 三处（共 4 处响应头模板）。

执行：

```bash
sed -i.bak 's/Access-Control-Allow-Headers: Content-Type, Authorization\\r\\n/Access-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token\\r\\n/g' crates/codex-assistant-core/src/launcher.rs && rm crates/codex-assistant-core/src/launcher.rs.bak
```

验证 4 处全部修改：

```bash
grep -c "X-Codex-Helper-Token" crates/codex-assistant-core/src/launcher.rs
```

Expected: ≥ 5（4 处响应头 + 401 分支中的 1 处）。

- [ ] **Step 6: 重跑测试**

Run: `cargo test -p codex-assistant-core --test helper_token`
Expected: 4 passed

- [ ] **Step 7: 跑整套核心测试确保未回归**

Run: `cargo test -p codex-assistant-core`
Expected: 全部通过；尤其 `relay_payload_does_not_expose_token_text` 保持绿。

- [ ] **Step 8: Commit**

```bash
git add crates/codex-assistant-core/src/launcher.rs crates/codex-assistant-core/tests/helper_token.rs
git -c commit.gpgsign=false commit -m "feat(launcher): enforce X-Codex-Helper-Token on bridge"
```

---

## Task 5: 渲染端注入脚本带上 token

**Files:**
- Modify: `assets/inject/renderer-inject.js`

- [ ] **Step 1: 在脚本顶部读取 token 并定义 helperFetch**

打开 `assets/inject/renderer-inject.js`，在第 2 行 `const helperBase = window.__CODEX_SESSION_DELETE_HELPER__ || "http://127.0.0.1:57321";` 之后插入：

```javascript
  const helperToken = window.__CODEX_ASSISTANT_HELPER_TOKEN__ || "";
  async function helperFetch(path, init = {}) {
    const headers = new Headers(init.headers || {});
    if (helperToken) headers.set("X-Codex-Helper-Token", helperToken);
    return fetch(`${helperBase}${path}`, { ...init, headers });
  }
```

- [ ] **Step 2: 替换 fetch 调用点（共 3 处）**

读取相邻上下文后逐处替换。

A. `assets/inject/renderer-inject.js:2313` 的 `fetch(\`${helperBase}/diagnostics/log\`, {` 改为 `helperFetch("/diagnostics/log", {`，把 init 对象中显式构造的 URL 删掉（init 不再含 URL）。

具体编辑：定位到 `fetch(\`${helperBase}/diagnostics/log\`, {` 一行，连同 `};` 结束的整块替换为：

```javascript
    helperFetch("/diagnostics/log", {
      method: "POST",
      keepalive: true,
      headers: { "Content-Type": "application/json" },
      body,
    }).catch(() => {});
```

B. `assets/inject/renderer-inject.js:2949` 的 `const response = await fetch(\`${helperBase}${path}\`, {` 改为：

```javascript
        const response = await helperFetch(path, {
```

并把原 fetch 调用末尾的 `\`${helperBase}${path}\`,` 实参一并删掉（helperFetch 只接收 path）。

C. `assets/inject/renderer-inject.js:2970` 同理：

```javascript
        const response = await helperFetch(path, {
```

- [ ] **Step 3: 删除 sendBeacon 分支**

打开 `assets/inject/renderer-inject.js`，定位 `sendCodexPlusDiagnostic` 函数（约 2301 行）。把：

```javascript
    const body = JSON.stringify(payload);
    try {
      if (navigator.sendBeacon) {
        const blob = new Blob([body], { type: "application/json" });
        if (navigator.sendBeacon(`${helperBase}/diagnostics/log`, blob)) return;
      }
    } catch (_) {}
    fetch(`${helperBase}/diagnostics/log`, {
      // … 原本配置
    }).catch(() => {});
```

整段替换为：

```javascript
    const body = JSON.stringify(payload);
    helperFetch("/diagnostics/log", {
      method: "POST",
      keepalive: true,
      headers: { "Content-Type": "application/json" },
      body,
    }).catch(() => {});
```

`navigator.sendBeacon` 不能携带自定义请求头；`fetch(..., {keepalive: true})` 覆盖页面卸载场景，且单条 payload 远低于 64 KiB 限额。

- [ ] **Step 4: 静态扫描确认无遗漏**

Run: `grep -n "fetch(\`\${helperBase}" assets/inject/renderer-inject.js`
Expected: 无任何输出（所有调用都已走 helperFetch）。

Run: `grep -n "sendBeacon" assets/inject/renderer-inject.js`
Expected: 无任何输出。

- [ ] **Step 5: 重新构建确保 lib 测试仍过（注入脚本作为字符串 include）**

Run: `cargo test -p codex-assistant-core --test cdp_bridge`
Expected: 通过。

- [ ] **Step 6: Commit**

```bash
git add assets/inject/renderer-inject.js
git -c commit.gpgsign=false commit -m "feat(inject): send X-Codex-Helper-Token from renderer fetches"
```

---

## Task 6: update.rs 增加 sha256 字段与解析

**Files:**
- Modify: `crates/codex-assistant-core/src/update.rs`

- [ ] **Step 1: 写失败的单元测试**

在 `crates/codex-assistant-core/src/update.rs` 文件末尾（如已有 `#[cfg(test)] mod tests` 就追加；否则新建）：

```rust
#[cfg(test)]
mod sha256_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn latest_json_parses_sha256() {
        let payload = json!({
            "version": "1.4.0",
            "assets": [{
                "name": "codex-assistant_1.4.0_x64-setup.exe",
                "url": "https://example.com/x.exe",
                "sha256": "ab".repeat(32)
            }]
        });
        let release = release_from_latest_json_payload(&payload).expect("parse");
        assert_eq!(release.asset_name.as_deref(), Some("codex-assistant_1.4.0_x64-setup.exe"));
        assert_eq!(release.asset_sha256.as_deref(), Some(&*"ab".repeat(32)));
    }

    #[test]
    fn latest_json_missing_sha256_yields_none() {
        let payload = json!({
            "version": "1.4.0",
            "assets": [{
                "name": "codex-assistant_1.4.0_x64-setup.exe",
                "url": "https://example.com/x.exe"
            }]
        });
        let release = release_from_latest_json_payload(&payload).expect("parse");
        assert!(release.asset_sha256.is_none());
    }

    #[test]
    fn verify_matches_real_sha256() {
        let body = b"hello world";
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        verify_asset_sha256(expected, body).expect("matches");
    }

    #[test]
    fn verify_rejects_mismatch() {
        let body = b"hello world";
        let expected = "00".repeat(32);
        let err = verify_asset_sha256(&expected, body).unwrap_err();
        assert!(err.to_string().contains("校验失败"), "{err}");
    }

    #[test]
    fn verify_rejects_bad_length() {
        assert!(verify_asset_sha256("abc", b"x").is_err());
    }

    #[test]
    fn verify_rejects_non_hex() {
        let expected = format!("{}gg", "a".repeat(62));
        assert!(verify_asset_sha256(&expected, b"x").is_err());
    }
}
```

- [ ] **Step 2: 运行测试验证失败（类型/函数还不存在）**

Run: `cargo test -p codex-assistant-core sha256_tests`
Expected: 编译错误，`asset_sha256` 字段和 `verify_asset_sha256` 函数未定义。

- [ ] **Step 3: 添加 sha256 字段**

修改 `crates/codex-assistant-core/src/update.rs:10-23` 的 `ReleaseAsset` 和 `Release` 结构：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub body: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    #[serde(default)]
    pub asset_sha256: Option<String>,
}
```

- [ ] **Step 4: 把 sha256 引入解析路径**

修改 `release_from_latest_json_payload`（约 106 行起）中 assets 收集部分。在收集 `(name, url)` 改为收集 `(name, url, sha256)`：

```rust
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            let name = asset.get("name")?.as_str()?.to_string();
            let url = asset
                .get("url")
                .or_else(|| asset.get("browser_download_url"))?
                .as_str()?
                .to_string();
            let sha256 = asset
                .get("sha256")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_ascii_lowercase);
            Some((name, url, sha256))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("url")
            .or_else(|| payload.get("html_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .or_else(|| payload.get("release_summary"))
            .or_else(|| payload.get("notes"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.name.clone()),
        asset_url: selected.as_ref().map(|asset| asset.browser_download_url.clone()),
        asset_sha256: selected.and_then(|asset| asset.sha256),
    })
```

同步修改 `release_from_github_payload`（约 70-104 行）：

```rust
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            Some((
                asset.get("name")?.as_str()?.to_string(),
                asset.get("browser_download_url")?.as_str()?.to_string(),
                None::<String>,
            ))
        })
        .collect::<Vec<_>>();
    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        url: payload
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.name.clone()),
        asset_url: selected.as_ref().map(|asset| asset.browser_download_url.clone()),
        asset_sha256: selected.and_then(|asset| asset.sha256),
    })
```

同步修改 `select_update_asset` 签名和返回（约 149 行起）：

```rust
pub fn select_update_asset(assets: &[(String, String, Option<String>)]) -> Option<ReleaseAsset> {
    let named = assets
        .iter()
        .filter(|(name, url, _)| !name.trim().is_empty() && !url.trim().is_empty())
        .collect::<Vec<_>>();
    for (name, url, sha256) in &named {
        let lower = name.to_ascii_lowercase();
        if platform_asset_rank(&lower) == 0 {
            return Some(ReleaseAsset {
                name: (*name).clone(),
                browser_download_url: (*url).clone(),
                sha256: sha256.clone(),
            });
        }
    }
    None
}
```

- [ ] **Step 5: 新增 `verify_asset_sha256`**

在 `crates/codex-assistant-core/src/update.rs` 末尾（`launch_installer` 之后、`#[cfg(test)] mod` 之前）追加：

```rust
pub fn verify_asset_sha256(expected_hex: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    anyhow::ensure!(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "更新包校验失败：非法 sha256 长度或字符"
    );
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    anyhow::ensure!(actual == expected, "更新包校验失败：sha256 不匹配");
    Ok(())
}
```

- [ ] **Step 6: 运行测试**

Run: `cargo test -p codex-assistant-core sha256_tests`
Expected: 6 passed

- [ ] **Step 7: Commit**

```bash
git add crates/codex-assistant-core/src/update.rs
git -c commit.gpgsign=false commit -m "feat(update): parse and verify asset sha256"
```

---

## Task 7: perform_update 强制校验

**Files:**
- Modify: `crates/codex-assistant-core/src/update.rs`

- [ ] **Step 1: 写失败的集成测试**

在 `crates/codex-assistant-core/src/update.rs` 的 `sha256_tests` 模块末尾追加：

```rust
    use tempfile::TempDir;

    fn release_with_sha(sha: Option<&str>) -> Release {
        Release {
            version: "1.4.0".into(),
            url: "https://example.com".into(),
            body: "".into(),
            asset_name: Some("codex-assistant_1.4.0_x64-setup.exe".into()),
            asset_url: Some("https://example.com/x.exe".into()),
            asset_sha256: sha.map(str::to_string),
        }
    }

    #[test]
    fn validate_download_rejects_missing_sha256() {
        let dir = TempDir::new().unwrap();
        let release = release_with_sha(None);
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        let err = validate_downloaded_installer(&release, &installer, b"payload").unwrap_err();
        assert!(err.to_string().contains("缺少校验和"), "{err}");
        assert!(!installer.exists(), "installer should be removed on failure");
    }

    #[test]
    fn validate_download_rejects_mismatch() {
        let dir = TempDir::new().unwrap();
        let release = release_with_sha(Some(&"00".repeat(32)));
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        let err = validate_downloaded_installer(&release, &installer, b"payload").unwrap_err();
        assert!(err.to_string().contains("校验失败"), "{err}");
        assert!(!installer.exists());
    }

    #[test]
    fn validate_download_accepts_match() {
        let dir = TempDir::new().unwrap();
        // sha256("payload") = 239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5
        let release = release_with_sha(Some(
            "239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5",
        ));
        let installer = dir.path().join("x.exe");
        std::fs::write(&installer, b"payload").unwrap();
        validate_downloaded_installer(&release, &installer, b"payload").expect("ok");
        assert!(installer.exists());
    }
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p codex-assistant-core sha256_tests::validate_download`
Expected: 编译失败，`validate_downloaded_installer` 不存在。

- [ ] **Step 3: 抽出 `validate_downloaded_installer`**

在 `crates/codex-assistant-core/src/update.rs` 的 `verify_asset_sha256` 函数后追加：

```rust
pub fn validate_downloaded_installer(
    release: &Release,
    installer_path: &Path,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let expected = match release.asset_sha256.as_deref() {
        Some(sha) if !sha.trim().is_empty() => sha,
        _ => {
            let _ = std::fs::remove_file(installer_path);
            let _ = crate::diagnostic_log::append_diagnostic_log(
                "security.update_missing_sha256",
                serde_json::json!({
                    "version": release.version,
                    "asset_name": release.asset_name,
                }),
            );
            anyhow::bail!("更新包缺少校验和，已拒绝安装");
        }
    };
    if let Err(error) = verify_asset_sha256(expected, bytes) {
        let _ = std::fs::remove_file(installer_path);
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.update_sha256_mismatch",
            serde_json::json!({
                "version": release.version,
                "asset_name": release.asset_name,
                "error": error.to_string(),
            }),
        );
        return Err(error);
    }
    Ok(())
}
```

- [ ] **Step 4: 在 `perform_update` 中调用 validate**

修改 `perform_update`（约 193-216 行）。下载完成后、`launch_installer` 调用前插入校验：

```rust
pub async fn perform_update(
    release: &Release,
    download_dir: &Path,
) -> anyhow::Result<UpdateInstall> {
    let url = release
        .asset_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let bytes =
        crate::http_client::proxied_client(&format!("CodexAssistant/{}", crate::version::VERSION))?
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
    let installer_path = download_asset_to(release, &bytes, download_dir)?;
    validate_downloaded_installer(release, &installer_path, &bytes)?;
    launch_installer(&installer_path)?;
    Ok(UpdateInstall {
        release: release.clone(),
        installer_path,
        launched: true,
    })
}
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p codex-assistant-core sha256_tests`
Expected: 9 passed（之前 6 个 + 新 3 个）

- [ ] **Step 6: 运行整套核心测试**

Run: `cargo test -p codex-assistant-core`
Expected: 全部通过

- [ ] **Step 7: Commit**

```bash
git add crates/codex-assistant-core/src/update.rs
git -c commit.gpgsign=false commit -m "feat(update): require sha256 before launching installer"
```

---

## Task 8: script_market.rs 强制 sha256

**Files:**
- Modify: `crates/codex-assistant-core/src/script_market.rs`

- [ ] **Step 1: 写失败的测试**

在 `crates/codex-assistant-core/src/script_market.rs` 末尾追加：

```rust
#[cfg(test)]
mod hardening_tests {
    use super::*;
    use serde_json::json;

    fn script_obj(id: &str, with_sha: bool) -> Value {
        let mut obj = json!({
            "id": id,
            "name": id,
            "version": "1.0.0",
            "script_url": format!("https://example.com/{id}.js"),
        });
        if with_sha {
            obj["sha256"] = json!("aa".repeat(32));
        }
        obj
    }

    #[test]
    fn manifest_drops_entries_without_sha256() {
        let raw = json!({
            "version": 1,
            "scripts": [script_obj("good", true), script_obj("bad", false)],
        });
        let manifest = parse_market_manifest(raw).expect("parse");
        let ids: Vec<&str> = manifest.scripts.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["good"]);
    }

    #[test]
    fn verify_sha256_rejects_empty() {
        let script = MarketScript {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            version: "1".into(),
            author: String::new(),
            tags: vec![],
            homepage: String::new(),
            script_url: "https://example.com/x.js".into(),
            sha256: String::new(),
        };
        assert!(verify_sha256(&script, b"content").is_err());
    }

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let script = MarketScript {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            version: "1".into(),
            author: String::new(),
            tags: vec![],
            homepage: String::new(),
            script_url: "https://example.com/x.js".into(),
            sha256: "00".repeat(32),
        };
        assert!(verify_sha256(&script, b"content").is_err());
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p codex-assistant-core hardening_tests`
Expected: `manifest_drops_entries_without_sha256` 失败（当前 sha256 可选，两条都会保留）；`verify_sha256_rejects_empty` 失败（当前空值返回 Ok）。

- [ ] **Step 3: 修改 `parse_market_script` 让 sha256 必填**

修改 `crates/codex-assistant-core/src/script_market.rs:113-141` 的 `parse_market_script`：

```rust
fn parse_market_script(raw: Value) -> Option<MarketScript> {
    let id = required_string(&raw, "id")?;
    let name = required_string(&raw, "name")?;
    let version = required_string(&raw, "version")?;
    let script_url = required_string(&raw, "script_url")?;
    let sha256 = required_string(&raw, "sha256")?;
    Some(MarketScript {
        id,
        name,
        description: optional_string(&raw, "description"),
        version,
        author: optional_string(&raw, "author"),
        tags: raw
            .get("tags")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        homepage: optional_string(&raw, "homepage"),
        script_url,
        sha256,
    })
}
```

- [ ] **Step 4: 修改 `parse_market_manifest` 记录丢弃**

修改 `crates/codex-assistant-core/src/script_market.rs:36-58`：

```rust
pub fn parse_market_manifest(raw: Value) -> anyhow::Result<ScriptMarketManifest> {
    let version = raw.get("version").and_then(Value::as_u64).unwrap_or(1);
    let updated_at = raw
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let raw_scripts: Vec<Value> = raw
        .get("scripts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let raw_total = raw_scripts.len();
    let mut dropped_ids: Vec<String> = Vec::new();
    let scripts: Vec<MarketScript> = raw_scripts
        .into_iter()
        .filter_map(|entry| {
            let candidate_id = entry
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            match parse_market_script(entry) {
                Some(parsed) => Some(parsed),
                None => {
                    if !candidate_id.is_empty() {
                        dropped_ids.push(candidate_id);
                    }
                    None
                }
            }
        })
        .collect();
    if scripts.len() != raw_total {
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.script_market_dropped_no_sha256",
            serde_json::json!({
                "total": raw_total,
                "kept": scripts.len(),
                "dropped_ids": dropped_ids,
            }),
        );
    }
    Ok(ScriptMarketManifest {
        version,
        updated_at,
        scripts,
    })
}
```

- [ ] **Step 5: 修改 `verify_sha256` 不再容忍空值**

修改 `crates/codex-assistant-core/src/script_market.rs:159-171`：

```rust
fn verify_sha256(script: &MarketScript, content: &[u8]) -> anyhow::Result<()> {
    let expected = script.sha256.trim().to_ascii_lowercase();
    anyhow::ensure!(
        !expected.is_empty(),
        "script {} missing sha256",
        script.id
    );
    let actual = to_hex(&Sha256::digest(content));
    if actual != expected {
        let _ = crate::diagnostic_log::append_diagnostic_log(
            "security.script_market_sha256_mismatch",
            serde_json::json!({
                "id": script.id,
                "expected": expected,
                "actual": actual,
            }),
        );
        anyhow::bail!("script {} sha256 mismatch", script.id);
    }
    Ok(())
}
```

- [ ] **Step 6: `download_script` 改走 proxied_client**

修改 `crates/codex-assistant-core/src/script_market.rs:72-82`：

```rust
pub async fn download_script(url: &str) -> anyhow::Result<Vec<u8>> {
    let client = crate::http_client::proxied_client(&format!(
        "CodexAssistant/{}",
        crate::version::VERSION
    ))?;
    Ok(client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to request script {url}"))?
        .error_for_status()
        .with_context(|| format!("script download returned an error status {url}"))?
        .bytes()
        .await
        .context("failed to read script download body")?
        .to_vec())
}
```

如果 `crate::http_client::proxied_client` 不可见，先打开 `crates/codex-assistant-core/src/lib.rs` 确认 `pub mod http_client;` 已存在；如已存在保持不变。

- [ ] **Step 7: 运行测试**

Run: `cargo test -p codex-assistant-core hardening_tests`
Expected: 3 passed

- [ ] **Step 8: 跑整套核心测试**

Run: `cargo test -p codex-assistant-core`
Expected: 全部通过

- [ ] **Step 9: Commit**

```bash
git add crates/codex-assistant-core/src/script_market.rs
git -c commit.gpgsign=false commit -m "feat(market): require sha256 on every market script"
```

---

## Task 9: 全工作区构建 + 文档收口

**Files:**
- 验证现有所有改动

- [ ] **Step 1: 全工作区构建**

Run: `cargo build --workspace`
Expected: 全部目标编译通过

- [ ] **Step 2: 全工作区测试**

Run: `cargo test --workspace`
Expected: 全部通过；特别确认 `relay_payload_does_not_expose_token_text` 仍绿。

- [ ] **Step 3: 在 spec 文档末尾追加"实施完成"标记**

打开 `docs/superpowers/specs/2026-05-23-security-hardening-design.md`，在文件末尾追加一行：

```markdown

---

**实施状态**：已按本计划实现，见 commits f539dd7 之后的提交链。
```

- [ ] **Step 4: 最终 Commit**

```bash
git add docs/superpowers/specs/2026-05-23-security-hardening-design.md
git -c commit.gpgsign=false commit -m "docs(spec): mark security hardening implementation done"
```

---

## 自审

**Spec 覆盖**：

- §1 本地桥 token：Task 1–5 全覆盖（依赖、模块、注入、服务端校验、渲染端 fetch 包装）。
- §2 更新 sha256：Task 6–7 覆盖（字段+解析、validate+perform_update）。
- §3 市场 sha256：Task 8 覆盖（解析丢弃 + verify + download_script）。
- 错误处理总览表的 5 个 `security.*` 事件：`security.helper_token_invalid`（Task 4）、`security.update_missing_sha256`（Task 7）、`security.update_sha256_mismatch`（Task 7）、`security.script_market_dropped_no_sha256`（Task 8）、`security.script_market_sha256_mismatch`（Task 8）—— 全覆盖。
- 测试策略：每个 Task 都先写测试。
- 发版脚手架（`scripts/release_manifest.*`）：**spec 的次要项**，本计划不包含；服务端 manifest 升级是发布动作，不在客户端 PR 内完成。如需，可后续追加 Task。

**占位扫描**：无 TBD/TODO；每个 step 都有具体代码或具体命令。

**类型一致性**：
- `injection_script(helper_port: u16, helper_token: &str)` 在 Task 3/4 一致。
- `ReleaseAsset.sha256: Option<String>` / `Release.asset_sha256: Option<String>` 在 Task 6/7 一致。
- `select_update_asset(&[(String, String, Option<String>)])` 在 Task 6 一致使用。
- `verify_asset_sha256(&str, &[u8])` 在 Task 6/7 一致。
- `validate_downloaded_installer(&Release, &Path, &[u8])` 在 Task 7 一致。

**已知开放点**：服务端 `latest.json` 和市场 `index.json` 升级到带 sha256 的工作是发布运维任务，不在本计划内；spec 已在"发布顺序"小节说明流程。

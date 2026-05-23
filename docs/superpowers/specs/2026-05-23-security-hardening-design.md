# 安全加固升级设计（2026-05-23）

## 背景

安全审计在当前主干代码（commit b3cf974）上没发现后门或 API key 外泄，但识别了三个"加固"项。本设计把这三项一次性纳入实现：

1. **本地 HTTP 桥（默认端口 57321）**：仅监听 `127.0.0.1`，但响应统一 `Access-Control-Allow-Origin: *` 且没有任何鉴权。本机任意浏览器 tab 可直接 `POST /v1/responses` 触发中转，消耗用户中转额度（无法盗取 API key，因为 key 不会被回写）。
2. **自动更新**：`update.rs::perform_update` 下载完成后直接 `Command::new(path).spawn()`（Windows）或 `open path`（macOS），完整性只靠 GitHub TLS 链。若 peixl 账号被攻破，可推恶意安装包。
3. **脚本市场**：`MarketScript.sha256` 是可选字段；缺省即跳过校验（`verify_sha256` 返回 Ok）。peixl 账号被攻破可在 manifest 中替换 `script_url` 投递任意 JS 注入到 Codex 渲染端。

允许破坏性变更：服务端 `latest.json` 和脚本市场 `index.json` 与客户端同步升级到带 `sha256` 字段的新格式。

## 设计原则

- 客户端"硬拒绝"优先于警告。缺校验数据 → 拒绝执行，不静默通过。
- 安全失败统一写 `diagnostic_log`，事件名前缀 `security.*`。
- token 等敏感数据只通过 CDP 注入到 Codex 渲染端的 `window` 全局，**不写文件、不写日志**。

## 范围内 / 范围外

**范围内**：

- 本地桥 token 注入 + 头校验
- `latest.json` 增加 `sha256` 字段 + 客户端下载后校验
- 脚本市场 `index.json` `sha256` 改为必填 + 解析/安装两层校验
- 发版脚手架更新（生成 sha256 并写入 manifest）

**范围外**：

- 代码签名（Ed25519/minisign）：本期不做。代码结构保留扩展空间，但不实现公钥嵌入。
- Origin 白名单：注入 token 已经足够区分"Codex 渲染端"和"本机其他网页"，不再做 Origin 校验。
- `script_url` 同源限制：保留以便后期上 CDN/镜像。
- 第三方依赖收紧、SBOM、供应链扫描。

---

## 1. 本地桥 token + 头校验

### 协议

- 启动时生成 32 字节随机数，base64url 编码为 43 字符 token，存进程内 `OnceLock<String>`；进程退出即失效。
- token 通过 CDP `Page.addScriptToEvaluateOnNewDocument` 注入到 `window.__CODEX_PLUS_HELPER_TOKEN__`（与现有 `window.__CODEX_SESSION_DELETE_HELPER__` 同一通道）。
- 客户端发往 `http://127.0.0.1:<helper_port>/*` 时，HTTP 头携带 `X-Codex-Helper-Token: <token>`。
- 服务端在处理 `OPTIONS` 之外的所有方法时，先做常量时间比较；不匹配 → `401 Unauthorized`（响应体 `{"status":"failed","message":"unauthorized"}`），CORS 头照常返回避免浏览器把 401 当成网络层错误。
- `OPTIONS` 预检放行（不验 token），`Access-Control-Allow-Headers` 追加 `X-Codex-Helper-Token`。

### 受保护路径

`handle_helper_connection` 中所有非 OPTIONS 路由：

- `/backend/status`、`/backend/repair`
- `/diagnostics/log`
- `/v1/models`、`/models`
- `/v1/responses`、`/responses`、`/responses/compact`
- 其余 → 404（保持现状）

### 代码组件

- 新建 `crates/codex-plus-core/src/helper_auth.rs`：
  - `pub fn ensure_helper_token() -> &'static str`：首次调用生成；用 `getrandom`（新增依赖） 读取 32 字节随机数，`base64::engine::general_purpose::URL_SAFE_NO_PAD` 编码。
  - `pub fn verify_token(provided: &str) -> bool`：长度先比，再用常量时间比较（手写 XOR 累加，避免引新依赖；`subtle` 不引入）。
- `crates/codex-plus-core/src/launcher.rs::handle_helper_connection`：
  - 在 `read_http_request` 后解析头部，提取 `X-Codex-Helper-Token`。
  - 当 `method != "OPTIONS"`：未通过 → 直接写 401 + 关闭连接 + 写 `security.helper_token_invalid` 诊断日志（不要把 token 本身写日志，只记 `provided_len`）。
- `crates/codex-plus-core/src/assets.rs::injection_script`：签名改为 `injection_script(helper_port: u16, helper_token: &str) -> String`，注入 `window.__CODEX_PLUS_HELPER_TOKEN__`。
- 调用链跟随：
  - `crates/codex-plus-core/src/launcher.rs::inject_with_context`、`inject`
  - `apps/codex-plus-launcher/src/main.rs::try_inject_with_context`、`inject_with_context`
  - 凡是构造 `injection_script(helper_port)` 的地方都改为传 token；token 取自 `helper_auth::ensure_helper_token()`。
- `assets/inject/renderer-inject.js`：
  - 顶部新增 `const helperToken = window.__CODEX_PLUS_HELPER_TOKEN__ || "";`
  - 实现 `function helperFetch(path, init = {}) {` 把 `X-Codex-Helper-Token` 头并入 `init.headers`，调用 `fetch(`${helperBase}${path}`, init)`。
  - 替换三处 `fetch(\`${helperBase}…\`)`（行 2313、2949、2970）为 `helperFetch`。
  - **`sendBeacon` 无法携带自定义头**：把 `navigator.sendBeacon` 那一支砍掉，统一走 `helperFetch('/diagnostics/log', {method:'POST', keepalive:true, body:…})`；`keepalive: true` 能覆盖大多数页面卸载场景，且 `/diagnostics/log` 单条 payload ≪ 64 KiB 的 keepalive 限额。
  - sponsor 图片走 data-uri fallback，URL 形式仍然访问 helperBase 的静态文件——这条路径目前没有，是 fallback；保留原样（GET 静态文件不在 helper 路由白名单内，已经会 404，不构成额度问题）。

### 错误处理

- 401 响应体一律是 `{"status":"failed","message":"unauthorized"}`。
- 渲染端 `helperFetch` 收到 401 → 抛出 `Error("CODEX_HELPER_UNAUTHORIZED")`；上层调用点已存在 `.catch(() => {})` 兜底，UI 影响为零。

### 测试

- `helper_auth` 单测：token 长度 = 43、字符集 `[A-Za-z0-9_-]`、两次调用返回同一引用。
- `verify_token` 单测：相同 → true；长度不同 → false（且早返回但仍走完循环以避免分支泄漏）；同长度但末字节不同 → false。
- `handle_helper_connection` 集成测：起真实 listener，
  1. 无 token POST `/v1/responses` → 401。
  2. 错 token → 401。
  3. 正确 token + 上游 mock → 200。
  4. OPTIONS 无 token → 204，且 `Access-Control-Allow-Headers` 包含 `X-Codex-Helper-Token`。
- `relay_payload_does_not_expose_token_text` 现有用例继续通过。

---

## 2. 自动更新 SHA-256 校验

### 协议变更（破坏性）

`latest.json` 新格式：

```json
{
  "format_version": 2,
  "version": "1.4.0",
  "url": "https://github.com/peixl/CodexAssistant/releases/tag/v1.4.0",
  "body": "release notes…",
  "assets": [
    {
      "name": "codex-plus_1.4.0_x64-setup.exe",
      "url": "https://github.com/peixl/CodexAssistant/releases/download/v1.4.0/codex-plus_1.4.0_x64-setup.exe",
      "sha256": "ab12…64hex"
    },
    {
      "name": "codex-plus_1.4.0_aarch64.dmg",
      "url": "https://github.com/peixl/CodexAssistant/releases/download/v1.4.0/codex-plus_1.4.0_aarch64.dmg",
      "sha256": "cd34…64hex"
    }
  ]
}
```

每个 asset `sha256` **必填**；缺失或非 64 字符 hex → 客户端拒绝安装。

`format_version` 仅用于诊断日志，不影响解析逻辑（向前兼容意义不大，因为我们就是破坏性升级；保留字段方便排查老客户端连了新 manifest 的情况）。

### 代码组件

- `crates/codex-plus-core/src/update.rs`：
  - `ReleaseAsset` 增 `pub sha256: Option<String>`。
  - `Release` 增 `pub asset_sha256: Option<String>`。
  - `select_update_asset` 把 `sha256` 一并带出来。
  - `release_from_latest_json_payload`、`release_from_github_payload` 都尝试读 `asset.sha256`。
  - 新增 `pub fn verify_asset_sha256(expected_hex: &str, bytes: &[u8]) -> anyhow::Result<()>`：长度 64、全 hex、用 `sha2::Sha256` 计算实际值、不区分大小写比较；不匹配 → `anyhow::bail!("更新包校验失败")`。
  - `perform_update`：下载完成后：
    1. 若 `release.asset_sha256` 为 `None` 或空 → `bail!("更新包缺少校验和，已拒绝安装")`，写 `security.update_missing_sha256` 诊断日志。
    2. 计算实际 sha256，调用 `verify_asset_sha256`；失败 → 删除 `installer_path`、写 `security.update_sha256_mismatch` 日志、bail。
    3. 通过 → 现有 `launch_installer` 流程不变。
- `release_from_github_payload`（兜底 GitHub API 路径）：GitHub API 不带 sha256，因此此路径必然走破坏性拒绝。审计认为现在主路径就是 `latest.json`，这条 fallback 在新版本中只用于人肉调试，所以接受"GitHub API fallback 永远拒绝安装"作为预期行为，文档里说明。

### 发版脚手架

- `scripts/release_manifest.{sh,py}`（择一）：扫描 `dist/` 中按平台命名规范的 asset，计算 sha256，生成 `latest.json`。
- CI / release 流程文档（如有 `RELEASE.md`/`CONTRIBUTING.md`）补一节"如何发布 latest.json"。本期只新增脚本，不动 CI 配置。

### 测试

- `release_from_latest_json_payload` 单测：解析含 sha256 的 asset；解析缺 sha256 的 asset（结果 `asset_sha256 = None`）。
- `verify_asset_sha256` 单测：匹配 ok；不匹配 err；长度错 err；非 hex err。
- `perform_update` 集成测（mock HTTP server + 临时目录）：
  1. asset 无 sha256 → err 且文件未保留。
  2. asset sha256 错 → err 且下载文件被删。
  3. sha256 正确 → 成功（不真正 spawn 安装器，通过注入 `launch_installer` 的可测试点或抽出 `prepare_update` 子函数实现）。

### 兼容性

- 旧客户端读新 `latest.json`：旧客户端没有 sha256 字段，会忽略，行为同旧版本，无回归。
- 新客户端读旧 `latest.json`：缺 sha256 → 拒绝安装，UI 显示"更新包缺少校验和，已拒绝安装。请稍后重试"。
- 发布顺序见末节"发布顺序"。

---

## 3. 脚本市场强制 SHA-256

### 协议变更（破坏性）

脚本市场 `index.json` 中 `sha256` **必填**。空值/缺字段 → 解析阶段被丢弃。

### 代码组件

- `crates/codex-plus-core/src/script_market.rs`：
  - `MarketScript.sha256` 类型从 `String`（带 `#[serde(default)]`）改为 `String`，但 `parse_market_script` 改用 `required_string("sha256")`；不存在则 `None` → 该条目被过滤掉。
  - `parse_market_manifest` 中：丢弃前后对比数量差，差值 > 0 时写 `security.script_market_dropped_no_sha256` 诊断日志，附 `dropped_ids` 列表，方便上游运维定位漏填条目。
  - `verify_sha256`：删除 `expected.is_empty() → Ok` 早返回；空 → `anyhow::bail!("script {id} missing sha256")`。理论上经过解析过滤之后 sha256 不会为空，这是双层防御。
  - `download_script` 改走 `crate::http_client::proxied_client`（与 `update.rs` 一致），User-Agent `CodexAssistant/{VERSION}`。
- `install_market_script_content`：保持现状（已经先 `verify_sha256` 再写文件）。

### 测试

- `parse_market_manifest` 单测：含 sha256 的条目保留；缺/空 sha256 的条目丢弃。
- `verify_sha256` 单测：空 → err；不匹配 → err；匹配 → ok。
- `install_market_script_content` 单测：哈希不匹配时确保 `manager.user_script_path_for_market_id` 路径下文件未生成（用临时目录）。

### 兼容性

- 服务端 `index.json` 需要在客户端发版前完成 sha256 回填。所有现存条目逐一补值；本期 PR 不修 manifest，但 release notes 中提示"市场 manifest 已同步更新到 v2 格式"。

---

## 模块边界 / 接口

```
codex-plus-core
├── helper_auth.rs        // 新模块：进程内 token；ensure_helper_token / verify_token
├── launcher.rs           // 路由前增加 token 检查；inject 链路传 token
├── assets.rs             // injection_script(helper_port, helper_token)
├── update.rs             // ReleaseAsset.sha256；verify_asset_sha256；perform_update 强制校验
└── script_market.rs      // sha256 必填；verify_sha256 不再容忍空值

apps/codex-plus-launcher
└── main.rs               // 调用链跟随 injection_script 新签名

assets/inject
└── renderer-inject.js    // helperFetch wrapper；删 sendBeacon 分支
```

每个模块只暴露上面列出的少量函数；其他逻辑不动。`helper_auth` 是叶子模块，只依赖 `getrandom` + `base64`。

---

## 错误处理总览

| 场景 | 行为 | 用户可见效果 | 诊断日志事件 |
|---|---|---|---|
| 本地桥 token 不匹配 | 401 响应 | helperFetch 抛错；UI 兜底 catch | `security.helper_token_invalid` |
| 更新 asset 缺 sha256 | 拒绝安装 | "更新包缺少校验和，已拒绝安装" | `security.update_missing_sha256` |
| 更新 sha256 不匹配 | 删除已下载文件 + 拒绝 | "更新包校验失败" | `security.update_sha256_mismatch` |
| 市场条目缺 sha256 | 解析阶段丢弃 | 该条目在 UI 中不出现 | `security.script_market_dropped_no_sha256` |
| 市场脚本下载 sha256 不匹配 | install 失败 | "脚本签名校验失败" | `security.script_market_sha256_mismatch` |

---

## 测试策略

- **单元测试**：每个模块的纯函数（`helper_auth::verify_token`、`update::verify_asset_sha256`、`script_market::verify_sha256`、`parse_market_manifest`）。
- **集成测试**：
  - `handle_helper_connection` 起本机 listener，验证 4 个场景（无/错/正确 token、OPTIONS）。
  - `perform_update` mock HTTP 下载，验证 3 个场景。
- **手动验收**：
  1. 启动 launcher，外部 curl `POST /v1/responses` 无 token → 401；
  2. 渲染端调用同路径 → 200（说明 token 注入成功）；
  3. 准备一个 `latest.json` 缺 sha256 → 更新被拒，日志 `security.update_missing_sha256`；sha256 错 → 同理；
  4. 准备一个 `index.json` 一条缺 sha256、一条 sha256 错：缺的条目在列表不出现，错的安装时报错。
- **现有用例**：`relay_payload_does_not_expose_token_text` 必须继续通过。

---

## 新增依赖

- `getrandom = "0.2"`（用于 token 生成）。`base64` 和 `sha2` 已在工作区。
- `subtle`：**不引入**。常量时间比较手写 XOR 累加即可，避免供应链扩面。

---

## 发布顺序

1. **服务端 manifest 升级**（独立 commit，先于客户端发版）：
   - 在 `peixl/CodexAssistant/releases/.../latest.json` 旁边发一份 `latest-v2.json` 携带 sha256；保留旧 `latest.json` 不变。
   - 在 `peixl/CodexAssistantScriptMarket` 仓库的 `index.json` 中给每个 script 补 sha256。**注意**：补完 sha256 后老客户端仍可读（多余字段忽略），新客户端开始强校验。
2. **客户端发版**：本期改动合入并发版；客户端默认仍指向旧 `latest.json` URL（待第 3 步切换）。
3. **manifest 切换**：把客户端 `DEFAULT_LATEST_JSON_URL` 在下一个补丁版本里改成 `latest-v2.json`；或更稳妥的做法是把 `latest.json` 内容整体替换为 v2 格式（旧客户端读到多出来的 `sha256`/`format_version` 字段会忽略，无回归）。本期 PR 选后者，**第 1 步直接更新 `latest.json` 到 v2**，省一次发版。

---

## 风险与缓解

- **风险**：服务端 manifest 漏补 sha256 → 所有新客户端无法更新。
  - **缓解**：发版前用 `scripts/release_manifest.*` 检查 manifest；release notes 提示运维。
- **风险**：getrandom 在某些目标平台（如 musl）需要特殊 feature。
  - **缓解**：在 `Cargo.toml` 中指定 `getrandom = { version = "0.2", features = ["std"] }`，覆盖所有受支持平台。
- **风险**：`navigator.sendBeacon` 退役后，页面卸载时的诊断日志可能丢失（fetch keepalive 在部分 Electron/Chromium 版本上限制 64 KiB）。
  - **缓解**：诊断 payload 远小于上限；丢失少量页面卸载日志可接受。

---

## 不做的事

- 不做代码签名、不嵌入公钥、不引入 minisign。
- 不限制 `script_url` 同源。
- 不重构无关代码。

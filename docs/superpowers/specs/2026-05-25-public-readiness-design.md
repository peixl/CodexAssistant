# 2026-05-25 — CodexAssistant 开源化 & Windows 兼容性强化

## 背景

仓库目前是 GitHub private，受 Actions 配额耗尽影响 CI 持续红。代码侧最近 10+ 次提交围绕 Windows 启动器、安装器、CDP 注入的健壮性收尾。本工作的目标是把项目推到"达到最佳开源项目要求"的状态，并翻成 public。

## 目标

1. **CI 全绿** —— 当前红色源于私有仓库 Actions 配额耗尽；本地先证明代码本身能通过 CI；翻 public 后免费配额无限，CI 自动恢复。
2. **Windows 兼容性** —— 审计并清理任何意外的硬编码用户路径/盘符/账号，确保 `app_paths.rs`、`windows_integration.rs`、`launcher.rs`、`install/windows.rs`、NSIS installer 在多种 Windows 环境（不同盘、不同用户名、便携安装、AppX 安装）都能正常发现 Codex。
3. **依赖最新** —— 把 11 个开放的 dependabot PR 本地评估、按安全等级合入。
4. **开源体验** —— README、CONTRIBUTING、SECURITY、ISSUE 模板、LICENSE 完整且准确；badge 在 public 后渲染正常。
5. **可重复自测** —— `cargo fmt && cargo clippy -D warnings && cargo test --workspace`、`npm run check && npm run test && npm run vite:build` 在本地通过；Windows 专属测试通过 CI 验证。

## 非目标

- 不重构现有架构（如 `launcher.rs` 拆解、Tauri 边界重设计）。
- 不新增任何用户可见特性。
- 不改注入脚本对外 API。

## 工作切片

### Phase 1 — 本地 CI 模拟
跑 macOS 本地全套：fmt / clippy -D warnings / test / npm check / vitest / vite build。修任何 develop 上潜伏的非配额性回归。

### Phase 2 — Windows 路径与兼容性
- `crates/codex-plus-core/src/app_paths.rs:58` 把唯一一处 `C:\Program Files\WindowsApps` 字面量改为仅在 env vars 缺失时回退。
- 审计每个 `#[cfg(windows)]` 块对 env var 缺失的容错。
- 审计 `windows_integration.rs`、`launcher.rs`、`install/windows.rs`、`scripts/installer/` NSIS 是否有硬编码用户路径。
- 修复发现的问题；新增/扩展测试覆盖路径解析的"环境变量缺失"边界。

### Phase 3 — 依赖升级
按危险度从低到高顺序，每个 PR 拉到 worktree 本地评估：
- **低风险**（patch / 已验证）：`serde_json 1.0.149→1.0.150`、`sha2 0.10→0.11`、`tauri 2.11.1→.2`、`lucide-react 0→1.16`（如 API 兼容）、`tailwind-merge 2→3`、GitHub Actions 升级（checkout/setup-node/upload-artifact/download-artifact）。
- **中等**：`windows 0.61→0.62`、`vitest 3→4`、`getrandom 0.2→0.4`。
- **高风险**：`vite 6→8`、`typescript 5→6`（major bump，可能 break 配置 / 类型系统）。

每个 PR 本地 build + test 通过才合；否则在 PR 上留 comment 说明阻塞，关闭 / 留待后续。

### Phase 4 — 开源表面润色
- README 检查（badges 在 public 后正确加载、macOS Intel 列入 platform）。
- CONTRIBUTING build/test 步骤完整。
- SECURITY 上报渠道明确。
- LICENSE 版权行准确。

### Phase 5 — 推送 & 翻 public
- 所有改动落 develop，开 PR `develop → main`。
- 本地自测 + CI（一旦 public，CI 立即恢复）双重绿。
- `gh repo edit --visibility public --accept-visibility-change-consequences`。
- 合 PR，清理已合并的 dependabot 分支。

## 风险与缓解

- **macOS 上跑不到 Windows 专属测试**：本地能保证 `cfg(not(windows))` 全绿；Windows 测试由翻 public 后的 CI（windows-latest）兜底。手段：合 main 前在 `develop` PR 上贴 `ci:full` label 触发完整矩阵。
- **major dependency bumps**：每个独立验证，失败的不合，避免连锁回归。
- **public 不可逆**：所有 review/CI 通过后才翻；翻之前再次确认无敏感 fixture（之前 `d0d4a0e` 已脱敏过一次，但再审一遍 `tests/` 与 git history）。

## 成功标准

- macOS 本地 `cargo test --workspace` + 前端 `npm run check/test/vite:build` 全绿。
- CI（含 windows-latest）全绿。
- 至少 6 个 dependabot PR 合并；未合的有明确理由。
- 仓库 visibility = public。
- README badges 渲染正常，链接全部可访问。

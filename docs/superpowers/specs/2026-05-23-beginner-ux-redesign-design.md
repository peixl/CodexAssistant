# Beginner-Friendly UX Redesign — Codex Assistant

**Date:** 2026-05-23
**Owner:** peixl
**Status:** Approved (head-of-conversation 2026-05-23)
**Scope:** Frontend (`apps/codex-assistant-manager/src/`) full rewrite. Rust/Tauri backend unchanged.

## 1. Goal

把"Codex Assistant"重写成一个 **AI 小白也能一次点对** 的桌面入口。
首屏只有一个巨型按钮（"打开 ChatGPT，已为你增强"），所有增强默认开启，所有配置默认隐藏到右侧抽屉。

成功定义：
- 装机后 **0 配置** 就能用上 5 个核心能力。
- 首屏永远只有 1 个主操作按钮，永远点 1 次就行。
- 没有 Tab、侧栏、面包屑、向导步骤。
- 现有 Rust 命令签名不变，回归面只限渲染层。

## 2. 用户画像与场景

- **A. AI 小白**（主目标）：知道 ChatGPT，但不知道"中转"、"注入"、"watcher"。打开应用想立刻看到 ChatGPT。
- **B. 进阶用户**（次目标）：想换 API Key、装脚本、看日志。允许多点 1–2 次（抽屉折叠）。

## 3. 暴露给小白的 5 个核心功能

1. **打开 ChatGPT**（首屏巨型按钮：启动 + 自动注入）
2. **插件解锁**（默认开启，已自动生效）
3. **一键删除对话**（默认开启，注入到 ChatGPT 网页内）
4. **导出 Markdown**（默认开启，注入到 ChatGPT 网页内）
5. **自动更新**（后台静默；发现新版时按钮下方一行提示）

保留但隐藏到"更多设置"：脚本市场、Provider 同步、桌面快捷方式管理、诊断包导出、中转配置文件编辑、关于/重置。

## 4. 文案去技术化（小白文案表）

| 旧术语 | 新文案 |
|---|---|
| 中转注入 / Relay | 加速登录 |
| 纯 API 模式 | 使用我自己的 API Key |
| Watcher | 自动增强 |
| Entry Points | 桌面快捷方式 |
| 诊断日志 / Diagnostics | 反馈包 |
| Provider 同步 | 服务源同步（高级） |

所有文案统一从 `lib/text.ts` 单源输出，方便审阅与统一改文。

## 5. 信息架构

只有 3 层：

1. **Home（唯一主屏）**
2. **AccountDrawer**（右侧抽屉，2 选项）
3. **MoreDrawer**（右侧抽屉，6 个 `<details>` 折叠分组）

无 Tab、无侧栏、无导航。返回靠抽屉的 X 关闭。

### 5.1 Home 首屏 ASCII

```
┌──────────────────────────────────────────────────────┐
│  Codex Assistant                       [⚙ 更多]      │
│                                                      │
│      ┌────────────────────────────────────┐          │
│      │   🚀  打开 ChatGPT                  │          │
│      │       已为你增强                    │          │
│      └────────────────────────────────────┘          │
│                                                      │
│  ✓ 插件解锁  ✓ 一键删对话  ✓ 导出 Markdown            │
│  ✓ 自动更新                                          │
│                                                      │
│  发现新版本 v1.5    [立即更新]                        │
│                                                      │
│                                          [👤 账号]    │
└──────────────────────────────────────────────────────┘
```

### 5.2 AccountDrawer

```
┌─ 账号 ─────────────────────────────────────┐
│  ○ 使用 ChatGPT 账号（推荐，更稳定）        │
│      [打开登录页]                          │
│                                            │
│  ○ 使用我自己的 API Key                    │
│      API Key  [_________]                  │
│      Base URL [_________(可选)]            │
│      [保存并切换]                          │
│                                            │
│  当前模式：使用 ChatGPT 账号               │
└────────────────────────────────────────────┘
```

底层映射：`apply_relay_injection` / `apply_pure_api_injection` / `clear_relay_injection`。
高级中转 JSON 编辑下沉到 MoreDrawer。

### 5.3 MoreDrawer（默认全收起）

```
┌─ 更多设置 ────────────────────────────────┐
│ ▸ 增强能力（脚本市场）                     │
│ ▸ 服务源同步（高级）                       │
│ ▸ 桌面快捷方式 / 卸载                      │
│ ▸ 反馈包导出                               │
│ ▸ 高级：中转配置文件编辑                   │
│ ▸ 关于 / 重置                              │
└────────────────────────────────────────────┘
```

每分组是一个 `<details>`，关闭态只显示标题，无视觉噪音。

## 6. 启动按钮状态机

`useLauncherMachine` hook，5 态有限状态机：

| 状态 | 进入条件 | 按钮文案 | 视觉 |
|---|---|---|---|
| `ready` | watcher 已装 + 账号就绪 | 🚀 打开 ChatGPT · 已为你增强 | 蓝色实心 |
| `launching` | 用户点击后 | 正在打开… | 蓝色 + spinner，禁用 |
| `need_account` | watcher 装好但未注入账号 | ➜ 先登录 ChatGPT | 灰边 |
| `preparing` | 后台 install_watcher / apply_*injection 中 | 准备增强… | 蓝色 + spinner，禁用 |
| `error` | 任一后台动作失败 | ⚠ 再试一次 · 查看原因 | 红边 + 内联红字 |

转移规则：
- 进入 Home 时：依据 `load_overview` / `relay_status` / `load_watcher_state` 计算初态。
- 首次发现 `watcher.installed=false` 自动进 `preparing`，串行 `install_watcher` → `enable_watcher` → `apply_*injection`。全成功回 `ready`，任一失败进 `error`。
- 用户点击 ready：调 `launch_codex_plus`，进 `launching`；命令完成后回 `ready`（不阻塞）。
- 用户点击 error：清错误，重做最近一次失败动作，进 `preparing`。

## 7. 自动更新

- 启动后 5 秒触发 `check_update`（已存在）。
- 若 `update_available=true`：Home 在胶囊行下方多一行 "发现新版本 vX · 立即更新"，链接形态而非按钮，不喧宾夺主。
- 点击 → `perform_update`，弹一个小型 modal 显示下载进度（沿用现有 sha256 校验流，渲染层只 surface 后端中文错误）。
- 校验失败：modal 内显示后端原话 + "导出反馈包"快捷链接。

## 8. 错误与空态原则

- **永不弹 alert/modal 报错**（更新模态除外），错误一律内联在动作所在的卡片/按钮下方红字。
- **端口冲突 / 57321 被占**：报错文案为"端口被占用，正在尝试自动修复"，后台跑 `repair_backend`，3 秒未恢复才提示"请手动重启应用"。
- **未登录 ChatGPT**：`launch_codex_plus` 返回后不强制改状态，账号抽屉的"已登录"靠 `load_overview` 推断；用户回到 Home 时若仍 `need_account`，按钮显示引导态。
- **缺少 sha256**（自动更新）：沿用 `update.rs:validate_downloaded_installer` 的错误，渲染层显示"更新校验失败，已拒绝安装" + "反馈包导出"。

## 9. 文件结构（前端重写）

`apps/codex-assistant-manager/src/`：

```
src/
  main.tsx                       ← 入口（不改）
  App.tsx                        ← 薄壳，编排 store + drawer 开关
  styles.css                     ← 沿用 Tailwind，配色微调
  state/
    useBackend.ts                ← 所有 Tauri 调用 + 内存缓存（无 SWR 依赖）
    useLauncherMachine.ts        ← 5 态状态机
    useUpdateProbe.ts            ← 启动 5s 后 check_update
  screens/
    Home.tsx                     ← 首屏（巨按钮 + 胶囊 + 更新提示）
  drawers/
    AccountDrawer.tsx
    MoreDrawer.tsx
  panels/
    ScriptsPanel.tsx             ← 脚本市场（复用 refresh/install/enable/delete 命令）
    ProvidersPanel.tsx           ← Provider 同步
    EntryPointsPanel.tsx         ← install/uninstall/repair shortcuts
    DiagnosticsPanel.tsx         ← copy_diagnostics + read_latest_logs
    RelayAdvancedPanel.tsx       ← read_relay_files / save_relay_file / test_relay_profile
    AboutPanel.tsx               ← backend_version + reset_settings
  components/
    LauncherButton.tsx           ← 5 态按钮（依赖 useLauncherMachine）
    CapabilityChips.tsx          ← 胶囊行
    UpdateBanner.tsx             ← 更新提示一行
    AccountStatusCard.tsx        ← Home 右下角"账号"入口
    Drawer.tsx                   ← 通用抽屉（无 Radix 依赖，单一动画）
    ui/                          ← 现有 shadcn 组件保留（badge/button/card/input/label/textarea）
  lib/
    invoke.ts                    ← call<T>() + 错误规整为 { code, message }
    text.ts                      ← 全部小白文案常量
    utils.ts                     ← 现有保留
```

**每个新文件 ≤ 250 行**。现有 `App.tsx`（2952 行）整体作废。

后端 `commands.rs` 不动。

## 10. Tauri 命令复用清单

按面板分组，已存在的全部复用：

- **Launcher**：`load_overview`, `load_watcher_state`, `install_watcher`, `enable_watcher`, `launch_codex_plus`, `relay_status`, `apply_relay_injection`, `apply_pure_api_injection`, `clear_relay_injection`
- **Account**：`load_settings`, `save_settings`
- **Scripts**：`refresh_script_market`, `install_market_script`, `set_user_script_enabled`, `delete_user_script`
- **Providers**：`load_ccs_providers`, `import_ccs_providers`, `sync_providers_now`
- **EntryPoints**：`install_entrypoints`, `uninstall_entrypoints`, `repair_shortcuts`, `repair_backend`
- **Diagnostics**：`copy_diagnostics`, `read_latest_logs`
- **RelayAdvanced**：`read_relay_files`, `save_relay_file`, `test_relay_profile`
- **About**：`backend_version`, `reset_settings`
- **Update**：`check_update`, `perform_update`
- **Misc**：`open_external_url`, `startup_options`, `load_ads`（首屏暂不展示广告位，保留 IPC 不删后端）

## 11. 测试策略

- **状态机单测（vitest）**：`useLauncherMachine` 的 5 态切换，覆盖：ready→launching→ready、ready→preparing→error→preparing→ready、need_account→preparing 等关键路径。Mock `invoke`。
- **invoke 适配层单测**：错误对象 → UI 文案映射稳定。
- **手测脚本**：新增 `docs/superpowers/specs/2026-05-23-beginner-ux-redesign-manual-smoke.md`，覆盖装机首日 8 步（首次启动 / 切到 API Key / 装脚本 / 触发更新 / 端口冲突修复 / 卸载快捷方式 / 反馈包导出 / 重置）。
- **Rust 后端**：不动（256 测试已通过）。
- **构建验证**：`pnpm/npm run vite:build` + `cargo build --release` + `cargo test --workspace` 全过才算完。

## 12. 不做清单

- ❌ 不做多 Tab / 侧栏 / 面包屑
- ❌ 不做 onboarding 向导
- ❌ 不做 i18n（仅中文）
- ❌ 不动 Rust 命令签名
- ❌ 不做 Provider 切换的图形化 UI（沿用 textarea）
- ❌ 不引入新的前端依赖（不加 Radix / Zustand / SWR，组件靠 React 19 自身 + 现有 shadcn 子集）
- ❌ 不动现有安全约束（helper token、SHA-256 校验、script_market sha256 必填）

## 13. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 渲染层重写漏了某条隐藏路径 | 第 10 节命令清单为完备性检查表；写计划时每命令至少 1 处调用 |
| 状态机覆盖不全导致按钮卡死 | 第 6 节 5 态封闭枚举 + vitest 单测兜底 |
| 更新流回归（sha256 校验链） | Tauri 命令不动；前端仅透传 `assetSha256` |
| 进阶用户找不到旧功能 | MoreDrawer 6 分组 1:1 覆盖旧"高级"区，首页保留小入口 |

## 14. 验收清单

- [ ] 首屏只有 1 个主按钮 + 1 行胶囊 + 0~1 行更新提示
- [ ] 新装用户 0 配置即可点 1 次打开 ChatGPT
- [ ] 5 态按钮在 mock 下全部可达且 vitest 单测通过
- [ ] AccountDrawer 切到 API Key 后 `relay_status` 反映正确
- [ ] MoreDrawer 6 个折叠分组全部能展开并正常调用对应命令
- [ ] 自动更新发现新版后 Home 显示一行更新提示，点击进入下载 → 校验 → 安装链路
- [ ] `cargo test --workspace` 256 全过；`cargo clippy --workspace -- -D warnings` 0 警告；前端 `tsc` + `vite build` 通过

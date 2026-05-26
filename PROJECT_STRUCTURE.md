# CodexAssistant 项目结构完整分析

## 📋 项目基本信息

**项目名称**: CodexAssistant  
**版本**: 1.1.3  
**开源协议**: MIT  
**主要语言**: Rust (后端) + TypeScript/React (前端)  
**框架**: Tauri 2.x  
**平台**: Windows x64, macOS (arm64/x86_64)  
**代码量**: Rust ~13K 行，TypeScript ~1.5K 行

---

## 🏗️ 架构概览

```
CodexAssistant (桌面应用)
  ├── codex-plus-plus (静默启动器 - Rust Binary)
  │   └── 启动 Codex + CDP 注入 (无 UI)
  │
  ├── CodexAssistant Manager (Tauri GUI)
  │   ├── React 19 + TypeScript 前端
  │   └── Tauri 命令调用 Rust 后端
  │
  ├── codex-plus-core (Rust Crate)
  │   ├── CDP 客户端
  │   ├── 中转 (Relay) 配置管理
  │   ├── Provider 切换
  │   ├── 用户脚本管理
  │   └── HTTP Bridge (127.0.0.1:57321)
  │
  ├── codex-plus-data (Rust Crate)
  │   ├── SQLite 数据库操作
  │   ├── Markdown 导出
  │   ├── Provider Sync
  │   └── 事务备份/撤销
  │
  └── renderer-inject.js (注入脚本)
      └── 注入到 Codex 渲染进程，实现增强功能
```

---

## 📁 完整目录树

```
CodexAssistant/
│
├── 📄 核心配置文件
│   ├── README.md / README_EN.md              # 项目文档
│   ├── package.json                          # Node.js 根工作空间
│   ├── Cargo.toml                            # Rust 工作空间 (members + shared deps)
│   ├── CODE_OF_CONDUCT.md / CONTRIBUTING.md  # 社区文档
│   └── SECURITY.md                           # 安全政策
│
├── 📂 .github/
│   ├── ISSUE_TEMPLATE/                       # Issue 模板
│   │   ├── bug_report.yml
│   │   ├── feature_request.yml
│   │   └── config.yml
│   ├── PULL_REQUEST_TEMPLATE.md              # PR 模板
│   ├── dependabot.yml                        # 自动依赖更新
│   └── workflows/
│       ├── ci.yml                            # CI: fmt, clippy, test, build
│       └── release-assets.yml                # Release 自动签发
│
├── 📂 apps/
│   │
│   ├── codex-plus-launcher/                  # 静默启动器 (codex-plus-plus)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs (22KB)                # 启动逻辑、CDP 连接、脚本注入
│   │
│   └── codex-plus-manager/                   # Tauri 管理工具 (GUI)
│       ├── package.json                      # Node.js 配置
│       ├── tsconfig.json                     # strict: true
│       ├── vite.config.ts                    # 前端构建
│       ├── vitest.config.ts                  # 单元测试
│       ├── src/                              # React + TypeScript 源代码
│       │   ├── main.tsx                      # 应用入口
│       │   ├── App.tsx                       # 主应用 (标签页布局)
│       │   ├── components/                   # UI 组件库
│       │   │   ├── AccountStatusCard.tsx
│       │   │   ├── CapabilityChips.tsx
│       │   │   ├── Drawer.tsx
│       │   │   ├── LauncherButton.tsx
│       │   │   ├── UpdateBanner.tsx
│       │   │   └── ui/ (shadcn/ui 基础组件)
│       │   ├── drawers/                      # 模态对话框
│       │   │   ├── AccountDrawer.tsx
│       │   │   └── MoreDrawer.tsx
│       │   ├── panels/                       # 标签页内容
│       │   │   ├── AboutPanel.tsx
│       │   │   ├── CodexPathPanel.tsx
│       │   │   ├── DiagnosticsPanel.tsx
│       │   │   ├── EntryPointsPanel.tsx
│       │   │   ├── ProvidersPanel.tsx
│       │   │   ├── RelayAdvancedPanel.tsx
│       │   │   ├── ScriptsPanel.tsx
│       │   │   └── ThemePanel.tsx
│       │   ├── screens/
│       │   │   └── Home.tsx
│       │   ├── state/                        # 状态管理
│       │   │   ├── launcherMachine.ts (XState)
│       │   │   ├── useBackend.ts
│       │   │   ├── useLauncherMachine.ts
│       │   │   ├── useTheme.ts
│       │   │   └── useUpdateProbe.ts
│       │   └── lib/                          # 工具库
│       │       ├── invoke.ts
│       │       ├── rendererInjectContract.test.ts
│       │       ├── text.ts
│       │       └── utils.ts
│       │
│       └── src-tauri/                        # Tauri 后端 (Rust)
│           ├── Cargo.toml
│           ├── tauri.conf.json
│           └── src/
│               ├── main.rs
│               ├── commands.rs (64KB) - Tauri 命令集
│               └── install.rs
│
├── 📂 crates/
│   ├── codex-plus-core/                      # 核心库 (11.4K 行 Rust)
│   │   ├── src/
│   │   │   ├── launcher.rs                   # 启动逻辑
│   │   │   ├── cdp.rs                        # CDP 客户端
│   │   │   ├── bridge.rs                     # HTTP Bridge
│   │   │   ├── routes.rs                     # Bridge 路由
│   │   │   ├── relay_config.rs               # 中转配置
│   │   │   ├── models.rs                     # 模型定义
│   │   │   ├── user_scripts.rs               # 脚本管理
│   │   │   ├── settings.rs                   # 设置管理
│   │   │   ├── paths.rs / app_paths.rs       # 路径管理
│   │   │   ├── status.rs / ports.rs          # 状态检查
│   │   │   ├── update.rs                     # 自动更新
│   │   │   ├── http_client.rs                # HTTP 客户端
│   │   │   └── install/ (平台特定代码)
│   │   └── tests/
│   │
│   └── codex-plus-data/                      # 数据库库 (1.85K 行 Rust)
│       └── src/
│           ├── SQLite 操作
│           ├── Markdown 导出
│           ├── Provider Sync
│           └── 备份 & 恢复
│
├── 📂 assets/
│   └── inject/
│       └── renderer-inject.js                # 注入脚本 (核心功能)
│
├── 📂 scripts/
│   └── installer/
│       ├── macos/
│       │   └── package-dmg.sh
│       └── windows/
│           └── CodexAssistant.nsi
│
└── 📂 docs/
    ├── images/
    └── superpowers/
        ├── plans/
        └── specs/
```

---

## 🔑 核心功能模块

### 1. 静默启动器 (codex-plus-plus)
- **文件**: `apps/codex-plus-launcher/src/main.rs`
- **大小**: 22KB
- **功能**: 启动 Codex → 连接 CDP → 注入脚本

### 2. HTTP Helper Bridge
- **文件**: `crates/codex-plus-core/src/bridge.rs`
- **地址**: `127.0.0.1:57321`
- **功能**: 注入脚本与后端通信的 HTTP 接口

### 3. 中转系统 (Relay)
- **文件**: `crates/codex-plus-core/src/relay_config.rs`
- **功能**: 管理 `~/.codex/config.toml` 中的 Provider 配置

### 4. 用户脚本管理
- **文件**: `crates/codex-plus-core/src/user_scripts.rs`
- **功能**: 脚本 CRUD、启动后注入

### 5. 数据库操作
- **文件**: `crates/codex-plus-data/src/`
- **目标**: `~/.codex/state_5.sqlite`
- **功能**: Markdown 导出、Provider Sync、备份恢复

### 6. Tauri 管理工具 GUI
- **入口**: `apps/codex-plus-manager/src/App.tsx`
- **框架**: React 19 + TypeScript
- **功能**: 标签页布局，包含 8 个管理面板

---

## 📊 代码统计

| 组件 | 代码行数 | 语言 |
|------|----------|------|
| codex-plus-core | 11,426 | Rust |
| codex-plus-data | 1,850 | Rust |
| Tauri 命令集 | 64KB | Rust |
| 启动器 | 22KB | Rust |
| React UI | ~1,500+ | TypeScript |

**总计**: ~13,276 行 Rust + ~1,500+ 行 TypeScript

---

## 🛠️ 主要技术栈

**Rust**:
- tokio (异步)
- reqwest (HTTP)
- rusqlite (SQLite)
- tokio-tungstenite (WebSocket/CDP)
- serde (序列化)

**前端**:
- React 19
- TypeScript 6.0
- Tauri 2.0
- Tailwind CSS 4.0
- Vite 8.0

---

## 🔄 开发流程

```bash
# 安装依赖
npm --prefix apps/codex-plus-manager ci

# 构建前端
npm --prefix apps/codex-plus-manager run vite:build

# 开发模式
npm run dev

# 本地检查
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix apps/codex-plus-manager run check
npm --prefix apps/codex-plus-manager run test
```

---

## 📂 核心数据文件

- `~/.codex/config.toml` - Codex 主配置 (Provider 写入位置)
- `~/.codex/state_5.sqlite` - 会话数据库
- `~/.codex/auth.json` - 官方登录态
- `~/.codex-session-delete/` - CodexAssistant 状态 & 日志

---

**文档生成时间**: 2026-05-26  
**项目版本**: 1.1.3

<div align="center">

<img src="docs/images/codex-plus-plus.svg" alt="CodexAssistant — hand-drawn by peixl / IFQ.AI" width="128">

# CodexAssistant

**面向 Codex App 的外部增强启动器与管理工具**

中文 · [English](README_EN.md)

[![Release](https://img.shields.io/github/v/release/peixl/CodexAssistant?style=flat-square)](https://github.com/peixl/CodexAssistant/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/peixl/CodexAssistant/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/peixl/CodexAssistant/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/peixl/CodexAssistant?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.x-24C8DB?style=flat-square)](https://tauri.app/)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-blue?style=flat-square)](#平台支持)

[安装](#安装) · [架构](#架构) · [开发](#开发) · [常见问题](#常见问题) · [贡献](CONTRIBUTING.md)

</div>

---

CodexAssistant 是一个对 [Codex App](https://chatgpt.com/codex) 的**外部增强工具**：它不会修改 Codex 的安装文件 (`app.asar`、可执行文件等)，而是通过 Chromium DevTools Protocol (CDP) 在 Codex 渲染进程中按需注入脚本，实现插件入口解锁、会话删除、Markdown 导出、自定义中转、用户脚本等能力。

IFQ.AI 的产品宣言很简单：**把高频 AI 工作做成可信赖的桌面工具**。CodexAssistant 因此只面向 Windows 与 macOS 主流桌面环境打磨体验、安装包和 CI。

整体由 **Rust 静默启动器 + Tauri (Rust + React) 管理工具 + 注入脚本** 三部分组成，面向 Windows 与 macOS (Intel / Apple Silicon)。

## ✨ 特性

- 🚀 **零侵入注入** — 通过 CDP 向已启动的 Codex 注入增强脚本，不动 Codex 的原始安装文件，不写 DLL 到 Codex 目录。
- 🔌 **中转 (Relay) 注入** — 在 `~/.codex/config.toml` 中以独立 provider 写入兼容 OpenAI Responses API 的中转配置，多套配置一键切换，支持随时清除并退回官方 ChatGPT 登录态。
- ⚡ **静默启动器** — 独立的 `codex-assistant` 二进制以最小开销启动 Codex，Windows 无控制台黑框，macOS 隐藏 Dock 图标，提供单实例守卫。
- 🎛️ **Tauri 管理工具** — React 19 + TypeScript (strict) 前端 + Rust 后端，含诊断、日志、设置、中转管理、用户脚本、Provider Sync 等面板，支持深浅主题切换。
- 🧩 **增强能力** — 插件入口解锁、强制安装特殊插件、会话删除、Markdown 导出、项目移动、Timeline、推荐内容。
- 📜 **用户脚本** — 独立管理用户自定义脚本，在 Codex 启动后按需注入。
- 🔄 **Provider Sync** — 切换中转/官方账号时，重写本地 SQLite 中的 provider 元数据，保证旧会话仍可见。
- 🌐 **Zed Remote 集成** — 识别远程 SSH 上下文，从 Codex 中直接打开远程文件到 Zed Remote Development。
- 🔁 **自动更新** — 管理工具与静默启动器均接入 GitHub Releases，发现新版本时引导更新。
- 📦 **官方打包** — Windows NSIS 安装程序，macOS Apple Silicon DMG，全部通过 GitHub Actions 自动签发。

## 📥 安装

从 [Releases](https://github.com/peixl/CodexAssistant/releases) 下载与平台对应的安装包：

| 平台 | 资产 |
| :--- | :--- |
| Windows x64 | `CodexAssistant-<version>-windows-x64-setup.exe` |
| macOS Apple Silicon | `CodexAssistant-<version>-macos-arm64.dmg` |

安装完成后会出现两个入口：

- **`CodexAssistant`** — 静默启动入口。点击直接启动 Codex 并完成注入，**不会弹出任何窗口**。
- **`CodexAssistant 管理工具`** — Tauri 控制面板。用于检查注入状态、查看日志、配置中转、管理用户脚本与开关增强功能。

> macOS 当前未做 Apple Developer ID 签名/公证，首次启动若被 Gatekeeper 拦截，请到「系统设置 → 隐私与安全性」放行。

## 🏗️ 架构

```
┌─────────────────────────┐         ┌──────────────────────────┐
│  CodexAssistant 管理工具  │  IPC    │   codex-assistant.exe    │
│   (Tauri: Rust + React)  │◀──────▶│   静默启动器 (Rust binary)  │
└────────────┬─────────────┘  HTTP  └─────────────┬────────────┘
             │ tauri commands                     │ spawn + monitor
             ▼                                    ▼
   ┌──────────────────────────────────────────────────┐
   │            codex-assistant-core (Rust crate)           │
   │  · launcher / single-instance guard               │
   │  · CDP client + renderer-inject.js bootstrap      │
   │  · relay config writer & provider switcher        │
   │  · settings / paths / proxy / update / models     │
   │  · bridge.rs - 127.0.0.1 helper endpoint          │
   └────────────────┬─────────────────────────────────┘
                    │ uses
                    ▼
   ┌──────────────────────────────────────────────────┐
   │            codex-assistant-data (Rust crate)           │
   │  · SQLite adapter for ~/.codex/state_5.sqlite     │
   │  · Markdown export / Provider Sync                │
   │  · transactional backup + undo                    │
   └──────────────────────────────────────────────────┘
                    │
                    ▼
            ┌──────────────────┐
            │   Codex App       │   ←─ CDP attach
            │   (Electron)      │   ←─ assets/inject/renderer-inject.js
            └──────────────────┘
```

### 关键工程决策

| 决策 | 取舍 |
| :--- | :--- |
| **外部 CDP 注入而非 `app.asar` 改写** | 升级 Codex 不破坏注入；不在 Codex 安装目录写文件；Windows 上无需提权。 |
| **静默启动器与管理工具拆为两个二进制** | 启动 Codex 时不需要 React/WebView 运行时；管理工具按需打开。 |
| **Rust workspace + Tauri** | 一套核心 crate 同时被两个二进制复用；GUI 通过 Tauri 命令调用，无双语言序列化层。 |
| **本地 HTTP bridge (`127.0.0.1:57321`)** | 注入脚本与 Rust 后端解耦；管理工具与渲染脚本可共用相同 API。 |
| **平台边界收紧到 Windows / macOS** | 安装包、CI 与运行路径只围绕主流桌面平台维护，避免把未支持系统误写成可用目标。 |

## 🔌 中转注入

中转注入适合**已经登录官方 ChatGPT、但希望模型请求走自定义 OpenAI 兼容 API** 的场景。在管理工具的「中转注入」面板：

1. 确认已检测到 ChatGPT 登录状态。
2. 添加一条或多条中转配置 (Base URL + Key)。
3. 选择当前配置并点击应用。
4. 启动 `CodexAssistant`。

CodexAssistant 会在 `~/.codex/config.toml` 写入：

```toml
model_provider = "CodexAssistant"

[model_providers.CodexAssistant]
name = "CodexAssistant"
wire_api = "responses"
requires_openai_auth = true
base_url = "https://example.com/v1"
experimental_bearer_token = "sk-..."
```

点击「清除 API 模式」会移除上述 provider 配置，并恢复到官方 ChatGPT 登录模式。

## 📂 数据位置

| 文件 | 用途 |
| :--- | :--- |
| `~/.codex/config.toml` | Codex 主配置；CodexAssistant 在此写入 provider |
| `~/.codex/auth.json` | Codex 登录态 (官方 ChatGPT) |
| `~/.codex/state_5.sqlite` | Codex 本地会话数据库 |
| `~/.codex/backups_state/provider-sync` | Provider Sync 事务备份 |
| `~/.codex-session-delete/` | CodexAssistant 状态、日志、注入缓存 |

## 🛠️ 开发

### 工具链

- Rust 1.85+ (workspace 使用 `edition = "2024"`)
- Node.js 20+ 与 npm
- Windows 10/11 或 macOS 12+ 的官方 SDK / Xcode Command Line Tools

### 中国大陆网络环境构建

如果在中国大陆访问 GitHub 或 npm 下载较慢，可以通过镜像源加速构建环境。

**配置 npm 镜像：**
```bash
npm config set registry https://registry.npmmirror.com
```

**配置 Cargo 镜像：**
编辑 `~/.cargo/config.toml` 文件：
```toml
[source.crates-io]
replace-with = "tuna"
[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
```

配置完成后，按如下顺序列进行依赖拉取与打包：

### 构建

```bash
# 1. 安装前端依赖
npm --prefix apps/codex-assistant-manager ci

# 2. 构建前端 — tauri::generate_context! 在编译时读取 dist/
npm --prefix apps/codex-assistant-manager run vite:build

# 3. 编译两个核心二进制 (静默启动器与 Tauri 管理工具)
cargo build --release
```

### 开发模式运行管理工具

```bash
npm --prefix apps/codex-assistant-manager run dev
```

### 本地完整校验 (与 CI 相同)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix apps/codex-assistant-manager run check
npm --prefix apps/codex-assistant-manager run test
```

### 项目结构

```
CodexAssistant/
├── apps/
│   ├── codex-assistant-launcher/     静默启动器二进制 (codex-assistant)
│   └── codex-assistant-manager/      Tauri 管理工具
│       ├── src/                 React + TypeScript UI
│       └── src-tauri/           Tauri 命令与窗口管理
├── assets/inject/               注入到 Codex 渲染端的 JS
├── crates/
│   ├── codex-assistant-core/         启动、CDP、设置、中转、Provider、更新、bridge
│   └── codex-assistant-data/         SQLite 适配、Markdown 导出、Provider Sync
├── scripts/installer/
│   ├── macos/package-dmg.sh     macOS DMG 打包脚本
│   └── windows/CodexAssistant.nsi  Windows NSIS 安装脚本
└── .github/workflows/           CI 与 Release Assets 工作流
```

## 🚦 平台支持

| 平台 | 静默启动器 | 管理工具 | 安装包 | CI |
| :--- | :---: | :---: | :---: | :---: |
| Windows x64 | ✅ | ✅ | NSIS `.exe` | ✅ |
| macOS arm64 (Apple Silicon) | ✅ | ✅ | `.dmg` | ✅ |
| macOS x64 (Intel) | ✅ | ✅ | 源码构建 | ✅ |

> Apple Silicon 是当前的官方 macOS 分发架构。Intel Mac 可以从源码构建（`cargo build --release` 加 `--target x86_64-apple-darwin`）。CI 只覆盖 Windows 与 macOS；Linux / Ubuntu 不作为运行、开发或发布目标。

## ❓ 常见问题

### CodexAssistant 菜单没有出现

请确认你是从 `CodexAssistant` 入口启动，而不是原版 Codex。可在管理工具的「诊断」「日志」面板查看注入是否成功，关注 `renderer.script_loaded` 与 `bridge.request` 事件。

### 插件提示后端连接失败

先测试 helper 端点是否可达：

```bash
curl -X POST http://127.0.0.1:57321/backend/status -d '{}' -H 'Content-Type: application/json'
```

如果该接口正常响应，但注入脚本仍然报失败，通常是 CDP bridge 重连或脚本缓存问题；重启 CodexAssistant，或在管理工具中清除注入缓存。

### Windows 上 Codex 打开了但增强不可用

CodexAssistant 会优先打开 Codex 本体；如果 VPN、腾讯电脑管家、企业防火墙或 WFP 规则阻断 `127.0.0.1` 本机环回连接，本次启动会自动降级为“只打开 Codex，不附加本地增强”，避免因为诊断通道不可用而完全打不开。此时请在 VPN/防火墙/安全软件中允许 `localhost` / `127.0.0.1`，或启用分流/本地网络访问后重新启动。

CodexAssistant 不会自动修改系统网络、防火墙或安全软件配置；所有恢复动作都需要用户在对应软件中确认。

### macOS 提示「无法打开」「已损坏」

当前 Release 未做 Developer ID 签名/公证。请到「系统设置 → 隐私与安全性」放行。也可以执行：

```bash
xattr -dr com.apple.quarantine /Applications/CodexAssistant.app
xattr -dr com.apple.quarantine "/Applications/CodexAssistant 管理工具.app"
```

### Intel Mac 能用吗？

可以，但目前 Release 只签发 Apple Silicon (`macos-arm64.dmg`) 的成品。Intel Mac 用户需要从源码构建：

```bash
rustup target add x86_64-apple-darwin
npm --prefix apps/codex-assistant-manager ci
npm --prefix apps/codex-assistant-manager run vite:build
cargo build --release --target x86_64-apple-darwin -p codex-assistant-launcher -p codex-assistant
```

### 支持 Linux 吗？

不支持。CodexAssistant 当前专注 Windows 与 macOS，不发布 Linux 安装包，也不把 Linux / Ubuntu 作为 CI 或开发环境目标。

## 🤝 贡献

欢迎 PR 与 Issue。开始之前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，并在提交前跑通本地完整校验。安全相关问题请按 [SECURITY.md](SECURITY.md) 私下报告。所有参与者需遵守 [行为准则](CODE_OF_CONDUCT.md)。

## 📄 许可证

[MIT License](LICENSE) — © 2026 peixl / IFQ.AI

## ⚠️ 免责声明

CodexAssistant 是**第三方外部增强工具**，与 OpenAI / Codex 团队没有任何隶属关系，不修改 Codex App 的原始文件。Codex App 的页面结构变化可能导致注入脚本需要更新；使用本工具引发的任何账号、数据或服务问题由使用者自行承担。

<div align="center">

<img src="docs/images/codex-plus-plus.svg" alt="CodexAssistant — hand-drawn by peixl / IFQ.AI" width="128">

# CodexAssistant

**External enhancement launcher and manager for the Codex App**

[中文](README.md) · English

[![Release](https://img.shields.io/github/v/release/peixl/CodexAssistant?style=flat-square)](https://github.com/peixl/CodexAssistant/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/peixl/CodexAssistant/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/peixl/CodexAssistant/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/peixl/CodexAssistant?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.x-24C8DB?style=flat-square)](https://tauri.app/)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-blue?style=flat-square)](#platform-support)

[Install](#install) · [Architecture](#architecture) · [Development](#development) · [FAQ](#faq) · [Contributing](CONTRIBUTING.md)

</div>

---

CodexAssistant is an **external enhancement tool** for the [Codex App](https://chatgpt.com/codex). It never modifies the Codex installation (`app.asar`, binaries, …); instead, it attaches over the Chromium DevTools Protocol (CDP) and injects scripts into the Codex renderer on demand to unlock plugin entries, enable session deletion and Markdown export, route requests through custom relays, run user scripts, and more.

IFQ.AI's product manifesto is simple: **turn high-frequency AI work into dependable desktop tools**. CodexAssistant therefore focuses its experience, installers, and CI on Windows and macOS only.

The system is built from three pieces — a **Rust silent launcher**, a **Tauri (Rust + React) manager**, and the **renderer injection script** — and targets Windows and macOS (Intel / Apple Silicon).

## ✨ Features

- 🚀 **Zero-touch injection** — CDP-attaches to a running Codex process. No `app.asar` patching, no DLLs written into the Codex install directory.
- 🔌 **Relay injection** — Writes an OpenAI Responses or Chat Completions relay profile into `~/.codex/config.toml` as a dedicated provider; Chat Completions uses Codex's native `wire_api = "chat"` path by default, avoiding unnecessary localhost proxying.
- ⚡ **Silent launcher** — `codex-assistant`, a standalone Rust binary that spawns Codex with minimal overhead. No console window on Windows, no Dock icon on macOS, single-instance guard.
- 🎛️ **Tauri manager** — React 19 + TypeScript (strict) frontend with a Rust backend. Includes Diagnostics, Logs, Settings, Relay Injection, User Scripts, and Provider Sync panels with dark/light themes.
- 🧩 **Enhancements** — Plugin entry unlock, forced install for restricted plugins, session delete, Markdown export, project move, Timeline, recommended content.
- 📜 **User scripts** — Managed independently and injected after Codex starts.
- 🔄 **Provider Sync** — Rewrites provider metadata in the local SQLite DB when switching between relays / official accounts, keeping old sessions visible.
- 🌐 **Zed Remote integration** — Detects remote SSH context and opens the corresponding file in Zed Remote Development directly from Codex.
- 🔁 **Automatic updates** — Both the manager and the silent launcher check GitHub Releases and prompt when a newer version is available.
- 📦 **First-class installers** — Windows NSIS installer and macOS Apple Silicon DMG, all built by GitHub Actions.

## 📥 Install

Grab the right installer from [Releases](https://github.com/peixl/CodexAssistant/releases):

| Platform | Asset |
| :--- | :--- |
| Windows x64 | `CodexAssistant-<version>-windows-x64-setup.exe` |
| macOS Apple Silicon | `CodexAssistant-<version>-macos-arm64.dmg` |

You'll end up with two entry points:

- **`CodexAssistant`** — silent launcher. Starts Codex with injection enabled and **never opens a window**.
- **`CodexAssistant Manager`** — Tauri control panel for inspecting injection state, viewing logs, configuring relays, managing user scripts, and toggling enhancements.

> macOS packages are signed and verified after the final `.app` and `.dmg` are assembled. If a release environment does not have Apple Developer ID notarization configured, the first launch may still need approval from **System Settings → Privacy & Security**.

## 🏗️ Architecture

```
┌─────────────────────────┐         ┌──────────────────────────┐
│   CodexAssistant Manager │  IPC    │   codex-assistant.exe    │
│   (Tauri: Rust + React)  │◀──────▶│   Silent launcher (Rust)  │
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

### Engineering trade-offs

| Decision | Rationale |
| :--- | :--- |
| **External CDP injection instead of patching `app.asar`** | Codex upgrades don't break injection; nothing is written into the Codex install directory; Windows doesn't need elevation. |
| **Silent launcher split from the manager** | Starting Codex doesn't drag in the WebView runtime; the manager only opens on demand. |
| **Rust workspace + Tauri** | A single core crate is reused by both binaries; the GUI talks to Rust through Tauri commands with no double serialization layer. |
| **Local HTTP bridge on `127.0.0.1:57321`** | Provides the enhancement-script fallback API and legacy local-proxy compatibility; Chat Completions relay profiles no longer depend on it by default. |
| **Platform boundary is Windows / macOS only** | Installers, CI, and runtime paths stay aligned with the supported desktop platforms instead of presenting unsupported systems as viable targets. |

## 🔌 Relay Injection

Relay injection is for users **already logged into the official ChatGPT account in Codex** who want model traffic to flow through a custom OpenAI-compatible API. In the manager's "Relay Injection" panel:

1. Confirm the ChatGPT login status is detected.
2. Add one or more relay profiles (Base URL + Key).
3. Select the active profile and click **Apply**.
4. Launch `CodexAssistant`.

CodexAssistant writes the following into `~/.codex/config.toml`:

```toml
model_provider = "CodexAssistant"

[model_providers.CodexAssistant]
name = "CodexAssistant"
wire_api = "responses"
requires_openai_auth = true
base_url = "https://example.com/v1"
experimental_bearer_token = "sk-..."
```

Click **Clear API mode** to remove the relay provider and revert to the official ChatGPT login flow.

## 📂 Data Locations

| Path | Purpose |
| :--- | :--- |
| `~/.codex/config.toml` | Codex main config — CodexAssistant writes its provider here |
| `~/.codex/auth.json` | Codex login state (official ChatGPT) |
| `~/.codex/state_5.sqlite` | Codex local session database |
| `~/.codex/backups_state/provider-sync` | Provider Sync transactional backups |
| `~/.codex-session-delete/` | CodexAssistant state, logs, and injection cache |

## 🛠️ Development

### Toolchain

- Rust 1.85+ (the workspace uses `edition = "2026"`)
- Node.js 20+ and npm
- Windows 10/11 or macOS 12+ with the required OS SDK / Xcode Command Line Tools

### Remote Mirrors (China Mainland)

If accessing GitHub or npm in mainland China is slow, you can speed up the build environment via mirrors:

**npm Mirror:**
```bash
npm config set registry https://registry.npmmirror.com
```

**Cargo Mirror (TUNA):**
Edit or create `~/.cargo/config.toml` (Windows: `C:\Users\<user>\.cargo\config.toml`):
```toml
[source.crates-io]
replace-with = "tuna"
[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
```

### Build

```bash
# 1. Install frontend deps
npm --prefix apps/codex-assistant-manager ci

# 2. Build the frontend — tauri::generate_context! reads dist/ at compile time
npm --prefix apps/codex-assistant-manager run vite:build

# 3. Build all Rust artefacts (silent launcher + manager)
cargo build --release
```

### Run the manager in dev mode

```bash
npm --prefix apps/codex-assistant-manager run dev
```

### Full local check (same gates as CI)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix apps/codex-assistant-manager run check
npm --prefix apps/codex-assistant-manager run test
```

### Project layout

```
CodexAssistant/
├── apps/
│   ├── codex-assistant-launcher/     Silent launcher binary (codex-assistant)
│   └── codex-assistant-manager/      Tauri manager
│       ├── src/                 React + TypeScript UI
│       └── src-tauri/           Tauri commands & window management
├── assets/inject/               JS injected into the Codex renderer
├── crates/
│   ├── codex-assistant-core/         Launch, CDP, settings, relay, provider, update, bridge
│   └── codex-assistant-data/         SQLite adapter, Markdown export, Provider Sync
├── scripts/installer/
│   ├── macos/package-dmg.sh     macOS DMG packaging script
│   └── windows/CodexAssistant.nsi  Windows NSIS installer script
└── .github/workflows/           CI and Release Assets workflows
```

## 🚦 Platform Support

| Platform | Silent launcher | Manager | Installer | CI |
| :--- | :---: | :---: | :---: | :---: |
| Windows x64 | ✅ | ✅ | NSIS `.exe` | ✅ |
| macOS arm64 (Apple Silicon) | ✅ | ✅ | `.dmg` | ✅ |
| macOS x64 (Intel) | ✅ | ✅ | source build | ✅ |

> Apple Silicon is the official macOS binary target. Intel Macs can build from source (`cargo build --release --target x86_64-apple-darwin`). CI covers Windows and macOS only; Linux / Ubuntu is not a runtime, development, or release target.

## ❓ FAQ

### The CodexAssistant menu never appears

Make sure you launched from the `CodexAssistant` entry, not the original Codex shortcut. Open the manager's **Diagnostics** / **Logs** panels and look for `renderer.script_loaded` and `bridge.request` events to confirm injection succeeded.

### The plugin says the backend is disconnected

Test the helper endpoint first:

```bash
curl -X POST http://127.0.0.1:57321/backend/status -d '{}' -H 'Content-Type: application/json'
```

If the endpoint works but injection still reports failure, it's typically a CDP bridge reconnect or a script-cache issue. Restart CodexAssistant, or clear the injection cache from the manager.

### macOS says "cannot be opened" or "damaged"

Since 1.2.1, the final `.app` bundles and `.dmg` are re-signed and verified after packaging, which avoids invalid bundle signatures being surfaced by Gatekeeper as "damaged". If a release was not Developer-ID-notarised, allow the app from **System Settings → Privacy & Security**, or remove the quarantine attribute:

```bash
xattr -dr com.apple.quarantine /Applications/CodexAssistant.app
xattr -dr com.apple.quarantine "/Applications/CodexAssistant 管理工具.app"
```

### Does Intel Mac work?

Yes, but currently only Apple Silicon (`macos-arm64.dmg`) is published as a binary release. Intel Mac users need to build from source:

```bash
rustup target add x86_64-apple-darwin
npm --prefix apps/codex-assistant-manager ci
npm --prefix apps/codex-assistant-manager run vite:build
cargo build --release --target x86_64-apple-darwin -p codex-assistant-launcher -p codex-assistant
```

### Is Linux supported?

No. CodexAssistant currently focuses on Windows and macOS. It does not publish Linux installers, and Linux / Ubuntu is not a supported CI or development target.

## 🤝 Contributing

PRs and issues are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first and run the full local check before pushing. Security-related issues should go through the channels in [SECURITY.md](SECURITY.md). All contributors are expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## 📄 License

[MIT License](LICENSE) — © 2026 peixl / IFQ.AI

## ⚠️ Disclaimer

CodexAssistant is a **third-party enhancement tool** with no affiliation to OpenAI or the Codex team. It does not modify any original Codex App files. Future Codex App releases may change the page structure and require updates to the injection script. Any account, data, or service issue arising from using this tool is the user's own responsibility.

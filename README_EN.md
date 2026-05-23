<div align="center">

<img src="docs/images/codex-plus-plus.png" alt="CodexAssistant" width="128">

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

The system is built from three pieces — a **Rust silent launcher**, a **Tauri (Rust + React) manager**, and the **renderer injection script** — and ships for Windows and macOS (Intel + Apple Silicon).

## ✨ Features

- 🚀 **Zero-touch injection** — CDP-attaches to a running Codex process. No `app.asar` patching, no DLLs written into the Codex install directory.
- 🔌 **Relay injection** — Writes an OpenAI Responses-compatible relay profile into `~/.codex/config.toml` as a dedicated provider; switch between multiple relay profiles and revert to the official ChatGPT login mode with one click.
- ⚡ **Silent launcher** — `codex-plus-plus`, a standalone Rust binary that spawns Codex with minimal overhead. No console window on Windows, no Dock icon on macOS, single-instance guard.
- 🎛️ **Tauri manager** — React 19 + TypeScript (strict) frontend with a Rust backend. Includes Diagnostics, Logs, Settings, Relay Injection, User Scripts, and Provider Sync panels with dark/light themes.
- 🧩 **Enhancements** — Plugin entry unlock, forced install for restricted plugins, session delete, Markdown export, project move, Timeline, recommended content.
- 📜 **User scripts** — Managed independently and injected after Codex starts.
- 🔄 **Provider Sync** — Rewrites provider metadata in the local SQLite DB when switching between relays / official accounts, keeping old sessions visible.
- 🌐 **Zed Remote integration** — Detects remote SSH context and opens the corresponding file in Zed Remote Development directly from Codex.
- 🔁 **Automatic updates** — Both the manager and the silent launcher check GitHub Releases and prompt when a newer version is available.
- 📦 **First-class installers** — Windows NSIS installer and macOS dual-architecture DMGs (x64 / arm64), all built by GitHub Actions.

## 📥 Install

Grab the right installer from [Releases](https://github.com/peixl/CodexAssistant/releases):

| Platform | Asset |
| :--- | :--- |
| Windows x64 | `CodexAssistant-<version>-windows-x64-setup.exe` |
| macOS Intel | `CodexAssistant-<version>-macos-x64.dmg` |
| macOS Apple Silicon | `CodexAssistant-<version>-macos-arm64.dmg` |

You'll end up with two entry points:

- **`CodexAssistant`** — silent launcher. Starts Codex with injection enabled and **never opens a window**.
- **`CodexAssistant Manager`** — Tauri control panel for inspecting injection state, viewing logs, configuring relays, managing user scripts, and toggling enhancements.

> macOS builds are currently neither Developer-ID-signed nor notarised. If Gatekeeper blocks the first launch, allow it from **System Settings → Privacy & Security**.

## 🏗️ Architecture

```
┌─────────────────────────┐         ┌──────────────────────────┐
│   CodexAssistant Manager │  IPC    │   codex-plus-plus.exe    │
│   (Tauri: Rust + React)  │◀──────▶│   Silent launcher (Rust)  │
└────────────┬─────────────┘  HTTP  └─────────────┬────────────┘
             │ tauri commands                     │ spawn + monitor
             ▼                                    ▼
   ┌──────────────────────────────────────────────────┐
   │            codex-plus-core (Rust crate)           │
   │  · launcher / single-instance guard               │
   │  · CDP client + renderer-inject.js bootstrap      │
   │  · relay config writer & provider switcher        │
   │  · settings / paths / proxy / update / models     │
   │  · bridge.rs - 127.0.0.1 helper endpoint          │
   └────────────────┬─────────────────────────────────┘
                    │ uses
                    ▼
   ┌──────────────────────────────────────────────────┐
   │            codex-plus-data (Rust crate)           │
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
| **Local HTTP bridge on `127.0.0.1:57321`** | Decouples the injection script from the Rust backend; the manager and the renderer script share one API. |
| **Every enhancement is cfg-gated per platform** | Windows / macOS specific code paths never leak into the other build; Linux still passes `cargo check` for development purposes. |

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

- Rust 1.85+ (the workspace uses `edition = "2024"`)
- Node.js 20+ and npm
- macOS and Windows ship the required system SDKs; on Linux install Tauri's deps (`libwebkit2gtk-4.1-dev`, …)

### Build

```bash
# 1. Install frontend deps
npm --prefix apps/codex-plus-manager ci

# 2. Build the frontend — tauri::generate_context! reads dist/ at compile time
npm --prefix apps/codex-plus-manager run vite:build

# 3. Build all Rust artefacts (silent launcher + manager)
cargo build --release
```

### Run the manager in dev mode

```bash
npm --prefix apps/codex-plus-manager run dev
```

### Full local check (same gates as CI)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix apps/codex-plus-manager run check
npm --prefix apps/codex-plus-manager run test
```

### Project layout

```
CodexAssistant/
├── apps/
│   ├── codex-plus-launcher/     Silent launcher binary (codex-plus-plus)
│   └── codex-plus-manager/      Tauri manager
│       ├── src/                 React + TypeScript UI
│       └── src-tauri/           Tauri commands & window management
├── assets/inject/               JS injected into the Codex renderer
├── crates/
│   ├── codex-plus-core/         Launch, CDP, settings, relay, provider, update, bridge
│   └── codex-plus-data/         SQLite adapter, Markdown export, Provider Sync
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
| macOS x64 (Intel) | ✅ | ✅ | `.dmg` | ✅ |
| Linux | — | — | — | ✅ (lint & test) |

Linux is not a distribution target, but the workspace stays buildable and CI runs lint + tests there as the canonical dev / CI environment.

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

Current releases are not Developer-ID-signed/notarised. Allow the app from **System Settings → Privacy & Security**, or remove the quarantine attribute:

```bash
xattr -dr com.apple.quarantine /Applications/CodexAssistant.app
xattr -dr com.apple.quarantine "/Applications/CodexAssistant 管理工具.app"
```

### Does Intel Mac work?

Yes. Each release ships both `macos-x64.dmg` and `macos-arm64.dmg`; pick the one that matches your CPU.

### Can I run it on Linux?

The repo builds and passes lint / tests on Linux, but the Codex App itself has no Linux release — there is **nothing to inject into**. Linux is a development / CI platform only.

## 🤝 Contributing

PRs and issues are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first and run the full local check before pushing. Security-related issues should go through the channels in [SECURITY.md](SECURITY.md). All contributors are expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## 📄 License

[MIT License](LICENSE) — © 2026 peixl / IFQ.AI

## ⚠️ Disclaimer

CodexAssistant is a **third-party enhancement tool** with no affiliation to OpenAI or the Codex team. It does not modify any original Codex App files. Future Codex App releases may change the page structure and require updates to the injection script. Any account, data, or service issue arising from using this tool is the user's own responsibility.

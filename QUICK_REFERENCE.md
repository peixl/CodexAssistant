# CodexAssistant 快速参考

## 📍 核心文件位置速查

### Rust 核心库
| 功能 | 文件路径 |
|------|---------|
| 启动器入口 | `apps/codex-assistant-launcher/src/main.rs` |
| CDP 客户端 | `crates/codex-assistant-core/src/cdp.rs` |
| HTTP Bridge | `crates/codex-assistant-core/src/bridge.rs` |
| 中转配置 | `crates/codex-assistant-core/src/relay_config.rs` |
| 用户脚本 | `crates/codex-assistant-core/src/user_scripts.rs` |
| 数据库操作 | `crates/codex-assistant-data/src/` |
| Tauri 命令 | `apps/codex-assistant-manager/src-tauri/src/commands.rs` |

### React 前端
| 功能 | 文件路径 |
|------|---------|
| 应用入口 | `apps/codex-assistant-manager/src/main.tsx` |
| 主应用 | `apps/codex-assistant-manager/src/App.tsx` |
| 诊断面板 | `apps/codex-assistant-manager/src/panels/DiagnosticsPanel.tsx` |
| 中转管理 | `apps/codex-assistant-manager/src/panels/RelayAdvancedPanel.tsx` |
| 脚本管理 | `apps/codex-assistant-manager/src/panels/ScriptsPanel.tsx` |
| 状态管理 | `apps/codex-assistant-manager/src/state/useBackend.ts` |
| 启动器状态机 | `apps/codex-assistant-manager/src/state/launcherMachine.ts` |

### 注入脚本
| 功能 | 文件路径 |
|------|---------|
| 注入脚本 | `assets/inject/renderer-inject.js` |

---

## 🚀 常用命令

### 开发
```bash
npm run dev                    # 启动开发模式 (热更新)
npm run check                  # TypeScript 类型检查
npm run test                   # 前端单元测试
npm run test:watch            # 测试监听模式
```

### 构建
```bash
npm --prefix apps/codex-assistant-manager run vite:build   # 构建前端
cargo build --release                                  # 构建 Rust (发布)
cargo build                                            # 构建 Rust (调试)
```

### 验证
```bash
cargo fmt --all -- --check                  # 格式检查
cargo clippy --workspace --all-targets -- -D warnings  # 代码检查
cargo test --workspace                     # 单元测试
```

---

## 📊 项目规模

| 指标 | 数值 |
|------|------|
| Rust 核心库 | 11,426 行 |
| Rust 数据库库 | 1,850 行 |
| React 组件库 | ~1,500+ 行 |
| 源文件总数 | 60+ |
| 配置文件 | 10+ |

---

## 🔌 技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| Rust | 1.85+ | 后端核心 |
| Tauri | 2.x | 桌面应用框架 |
| React | 19.0 | UI 框架 |
| TypeScript | 6.0 | 类型系统 |
| Vite | 8.0 | 前端构建 |
| Tailwind | 4.0 | CSS 框架 |
| Node.js | 20+ | 前端工具链 |

---

## 🏗️ 模块依赖关系

```
Tauri 管理工具 (GUI)
  └── 调用 Tauri 命令 (commands.rs)
      └── 调用 codex-assistant-core
          ├── 启动器逻辑
          ├── CDP 客户端
          ├── HTTP Bridge
          ├── 中转配置
          └── 用户脚本

注入脚本 (renderer-inject.js)
  └── 与 HTTP Bridge (127.0.0.1:57321) 通信
      └── codex-assistant-core 处理请求
          └── codex-assistant-data 操作数据库
```

---

## 📂 数据存储

| 数据 | 位置 | 用途 |
|------|------|------|
| 配置 | `~/.codex/config.toml` | Provider 配置 |
| 登录态 | `~/.codex/auth.json` | ChatGPT 认证 |
| 会话 | `~/.codex/state_5.sqlite` | 本地会话数据 |
| 日志 | `~/.codex-session-delete/` | 诊断日志 |
| 备份 | `~/.codex/backups_state/provider-sync/` | 事务备份 |

---

## 🔧 配置文件

| 文件 | 位置 | 用途 |
|------|------|------|
| Rust 工作空间 | `Cargo.toml` | 模块定义、共享依赖 |
| Node.js 工作空间 | `package.json` | 脚本快捷方式 |
| TypeScript 配置 | `apps/codex-assistant-manager/tsconfig.json` | 类型检查 |
| Vite 配置 | `apps/codex-assistant-manager/vite.config.ts` | 前端构建 |
| Tauri 配置 | `apps/codex-assistant-manager/src-tauri/tauri.conf.json` | 窗口/菜单/安全 |
| GitHub Actions | `.github/workflows/ci.yml` | CI 流程 |

---

## 🎯 功能速查

| 功能 | 核心文件 | 说明 |
|------|---------|------|
| **启动 Codex** | `launcher.rs` | 单实例、无 UI、CDP 连接 |
| **脚本注入** | `cdp.rs`, `assets/inject/` | 通过 DevTools Protocol |
| **中转注入** | `relay_config.rs` | 修改 ~/.codex/config.toml |
| **HTTP 通信** | `bridge.rs`, `routes.rs` | 127.0.0.1:57321 |
| **脚本管理** | `user_scripts.rs` | 本地脚本 CRUD |
| **数据库** | `codex-assistant-data/` | SQLite 操作与导出 |
| **管理 UI** | `App.tsx`, `panels/` | 标签页布局 |
| **状态管理** | `useBackend.ts`, `launcherMachine.ts` | React Hooks + XState |
| **自动更新** | `update.rs`, `useUpdateProbe.ts` | GitHub Releases |

---

## 🔍 调试关键点

### 注入失败
```bash
# 检查 Helper 端点
curl -X POST http://127.0.0.1:57321/backend/status -d '{}' -H 'Content-Type: application/json'

# 查看诊断日志 (管理工具 → 诊断面板)
# 关注 renderer.script_loaded 和 bridge.request 事件
```

### Codex 启动问题
```bash
# 检查 Codex 路径
# 查看启动日志 (管理工具 → 诊断面板)
```

### 中转配置问题
```bash
# 检查配置文件
cat ~/.codex/config.toml

# 清除并重新添加中转配置
# (管理工具 → 中转注入面板)
```

---

## 📝 编码规范

- **Rust**: `cargo fmt` + `cargo clippy` + strict warnings
- **TypeScript**: `strict: true` in tsconfig.json
- **测试**: 每个主要模块配置单元测试
- **文档**: README.md, CONTRIBUTING.md, SECURITY.md

---

## 🚨 重要注意事项

1. **平台条件编译**: Windows/macOS 特定代码用 `#[cfg(...)]` 隔离
2. **零侵入**: 不修改 Codex 安装文件，只注入脚本
3. **数据安全**: 数据库操作前自动备份
4. **单实例**: 防止多个 Codex 实例同时启动
5. **自动更新**: 通过 GitHub Releases，无法离线使用

---

## 📚 关键文档

- `README.md` - 项目概览和使用指南
- `CONTRIBUTING.md` - 贡献指南
- `SECURITY.md` - 安全问题报告
- `PROJECT_STRUCTURE.md` - 详细项目结构 (本文档)
- `QUICK_REFERENCE.md` - 快速参考 (本文档)

---

**最后更新**: 2026-05-26  
**项目版本**: 1.2.0

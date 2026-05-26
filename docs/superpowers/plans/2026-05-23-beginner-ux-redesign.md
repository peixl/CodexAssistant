# Beginner-Friendly UX Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `apps/codex-assistant-manager/src/App.tsx`（2952 行）重写为以"巨型启动按钮 + 默认增强"为核心的极简前端，保留 5 个面向小白的功能并把进阶能力折叠到抽屉。

**Architecture:** 渲染层全新；后端 Tauri 命令面 0 改动。状态机用纯函数 reducer + 薄 hook 编排副作用，便于 vitest 单测。新文件全部 ≤ 250 行，按 `state/`、`screens/`、`drawers/`、`panels/`、`components/`、`lib/` 分层。

**Tech Stack:** React 19 + Vite 6 + Tailwind 4 + TypeScript 5.8 + Tauri 2 + vitest（新增）。

**Spec:** `docs/superpowers/specs/2026-05-23-beginner-ux-redesign-design.md`

---

## Prelude: 工作目录、约束、术语

**工作目录**：每个 `npm run xxx` 都必须从 `apps/codex-assistant-manager/` 下运行。
**命令前缀**：本计划里写的 `npm run X`，在仓库根目录执行时等价于
```bash
(cd apps/codex-assistant-manager && npm run X)
```

**保留的安全约束（来自前一个 PR）**：
- 不动 `crates/codex-assistant-core/src/launcher.rs`、`update.rs`、`script_market.rs` 的安全逻辑。
- `helperFetch` 头 `X-Codex-Helper-Token` 由后端注入 ChatGPT 页面，前端不直接调；本计划不涉及。
- 自动更新链路（sha256 校验）已在 PR 中实现，本计划仅做渲染层透传。

**Tauri 调用形状参考**：
- `launch_codex_assistant({ request: { appPath: string|null, debugPort: number, helperPort: number } })`
- `apply_relay_injection()` / `apply_pure_api_injection()` / `clear_relay_injection()` 无参
- `save_settings({ settings: BackendSettings })`
- `install_market_script({ id })`
- `set_user_script_enabled({ key, enabled })`
- `delete_user_script({ key })`
- `save_relay_file({ request: SaveRelayFileRequest })`
- `test_relay_profile({ profile: RelayProfile })`
- `uninstall_entrypoints({ options: InstallOptions })`
- `perform_update({ release })`

完整签名在 `apps/codex-assistant-manager/src-tauri/src/commands.rs`，调用样本在 `apps/codex-assistant-manager/src/App.tsx`（保留到 Task 11 才删）。

**5 态状态机术语**（贯穿全计划，禁止改名）：
```
type LauncherState =
  | { kind: "preparing" }                    // 后台 install_watcher / enable_watcher / apply_*injection 中
  | { kind: "ready" }                        // 一切就绪，按钮可点
  | { kind: "launching" }                    // launch_codex_assistant 调用中
  | { kind: "need_account" }                 // 未注入账号
  | { kind: "error"; message: string };      // 任一动作失败
```

```
type LauncherEvent =
  | { type: "probe_done"; result: ProbeResult }
  | { type: "prepare_start" }
  | { type: "prepare_done" }
  | { type: "prepare_failed"; message: string }
  | { type: "launch_click" }
  | { type: "launch_done" }
  | { type: "launch_failed"; message: string }
  | { type: "retry" };
```

---

## 文件结构

新增（按计划任务顺序）：

```
apps/codex-assistant-manager/
  vitest.config.ts                                      ← Task 1
  src/
    lib/
      text.ts                                           ← Task 2
      invoke.ts                                         ← Task 2
      invoke.test.ts                                    ← Task 2
    state/
      launcherMachine.ts                                ← Task 3
      launcherMachine.test.ts                           ← Task 3
      useLauncherMachine.ts                             ← Task 4
      useBackend.ts                                     ← Task 4
      useUpdateProbe.ts                                 ← Task 4
    components/
      Drawer.tsx                                        ← Task 5
      LauncherButton.tsx                                ← Task 5
      CapabilityChips.tsx                               ← Task 5
      UpdateBanner.tsx                                  ← Task 5
      AccountStatusCard.tsx                             ← Task 5
    screens/
      Home.tsx                                          ← Task 6
    drawers/
      AccountDrawer.tsx                                 ← Task 7
      MoreDrawer.tsx                                    ← Task 10
    panels/
      ScriptsPanel.tsx                                  ← Task 8
      ProvidersPanel.tsx                                ← Task 8
      EntryPointsPanel.tsx                              ← Task 8
      DiagnosticsPanel.tsx                              ← Task 9
      RelayAdvancedPanel.tsx                            ← Task 9
      AboutPanel.tsx                                    ← Task 9
docs/superpowers/specs/
  2026-05-23-beginner-ux-redesign-manual-smoke.md       ← Task 12
```

替换：`apps/codex-assistant-manager/src/App.tsx`（Task 11）。

---

## Task 1: 安装 vitest 并加最小配置

**Files:**
- Modify: `apps/codex-assistant-manager/package.json`
- Create: `apps/codex-assistant-manager/vitest.config.ts`

- [ ] **Step 1: 在 `apps/codex-assistant-manager/` 下安装 vitest（不需要 jsdom，因为我们只测纯函数）**

Run:
```bash
cd apps/codex-assistant-manager && npm install --save-dev vitest@^3.0.0
```
Expected: 完成、`package.json` 更新、`package-lock.json` 出现。

- [ ] **Step 2: 在 `package.json` 的 `scripts` 段追加 `test` 脚本**

Edit `apps/codex-assistant-manager/package.json`，在 `"vite:build": "vite build"` 一行后追加：
```json
,
    "test": "vitest run",
    "test:watch": "vitest"
```

- [ ] **Step 3: 写 `vitest.config.ts`（与 `vite.config.ts` 并列）**

Create `apps/codex-assistant-manager/vitest.config.ts`：
```ts
import { defineConfig } from "vitest/config";
import path from "node:path";

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
```

- [ ] **Step 4: 跑一次 `npm run test` 确认 0 个测试也能成功退出**

Run:
```bash
cd apps/codex-assistant-manager && npm run test
```
Expected: `No test files found` 或 `0 passed`，**退出码 0**。如果非 0，按报错修正 include 路径。

- [ ] **Step 5: 提交**

```bash
git add apps/codex-assistant-manager/package.json apps/codex-assistant-manager/package-lock.json apps/codex-assistant-manager/vitest.config.ts
git commit -m "chore(manager): add vitest for pure-function unit tests"
```

---

## Task 2: 文案常量 + invoke 错误规整 + 单测

**Files:**
- Create: `apps/codex-assistant-manager/src/lib/text.ts`
- Create: `apps/codex-assistant-manager/src/lib/invoke.ts`
- Create: `apps/codex-assistant-manager/src/lib/invoke.test.ts`

- [ ] **Step 1: 写文案常量 `text.ts`**

```ts
// apps/codex-assistant-manager/src/lib/text.ts
export const TEXT = {
  appName: "Codex Assistant",
  launcher: {
    ready: "打开 ChatGPT",
    readyHint: "已为你增强",
    launching: "正在打开…",
    needAccount: "先登录 ChatGPT",
    preparing: "准备增强…",
    errorPrefix: "再试一次",
    errorAction: "查看原因",
  },
  capabilities: {
    plugins: "插件解锁",
    deleteChats: "一键删对话",
    exportMd: "导出 Markdown",
    autoUpdate: "自动更新",
  },
  account: {
    title: "账号",
    chatgpt: "使用 ChatGPT 账号（推荐，更稳定）",
    apiKey: "使用我自己的 API Key",
    openLogin: "打开登录页",
    saveSwitch: "保存并切换",
    current: "当前模式",
  },
  more: {
    title: "更多设置",
    sections: {
      scripts: "增强能力（脚本市场）",
      providers: "服务源同步（高级）",
      entryPoints: "桌面快捷方式 / 卸载",
      diagnostics: "反馈包导出",
      relayAdvanced: "高级：中转配置文件编辑",
      about: "关于 / 重置",
    },
  },
  update: {
    available: (v: string) => `发现新版本 ${v}`,
    cta: "立即更新",
    failedTitle: "更新校验失败，已拒绝安装",
    diagnosticsCta: "导出反馈包",
  },
  errors: {
    portBusy: "端口被占用，正在尝试自动修复",
    portBusyFinal: "请手动重启应用",
    networkFailed: "网络不通，请检查代理",
    unknown: "出错了，请稍后再试",
  },
} as const;
```

- [ ] **Step 2: 写 invoke 封装 `invoke.ts`**

```ts
// apps/codex-assistant-manager/src/lib/invoke.ts
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { TEXT } from "./text";

export type NormalizedError = { code: string; message: string };

export function normalizeInvokeError(error: unknown): NormalizedError {
  if (typeof error === "string") {
    return { code: "string", message: error || TEXT.errors.unknown };
  }
  if (error && typeof error === "object") {
    const e = error as { message?: unknown; code?: unknown };
    const message = typeof e.message === "string" && e.message.length > 0
      ? e.message
      : TEXT.errors.unknown;
    const code = typeof e.code === "string" ? e.code : "object";
    return { code, message };
  }
  return { code: "unknown", message: TEXT.errors.unknown };
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(command, args);
}

export async function callSafe<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<{ ok: true; data: T } | { ok: false; error: NormalizedError }> {
  try {
    const data = await tauriInvoke<T>(command, args);
    return { ok: true, data };
  } catch (error) {
    return { ok: false, error: normalizeInvokeError(error) };
  }
}
```

- [ ] **Step 3: 写失败测试 `invoke.test.ts`**

```ts
// apps/codex-assistant-manager/src/lib/invoke.test.ts
import { describe, expect, it } from "vitest";
import { normalizeInvokeError } from "./invoke";

describe("normalizeInvokeError", () => {
  it("turns string into NormalizedError", () => {
    expect(normalizeInvokeError("boom")).toEqual({ code: "string", message: "boom" });
  });

  it("uses fallback message for empty string", () => {
    const r = normalizeInvokeError("");
    expect(r.code).toBe("string");
    expect(r.message).toBe("出错了，请稍后再试");
  });

  it("reads object .message and .code", () => {
    expect(normalizeInvokeError({ message: "x", code: "TAURI_X" })).toEqual({
      code: "TAURI_X",
      message: "x",
    });
  });

  it("falls back to unknown for null/undefined", () => {
    expect(normalizeInvokeError(null).code).toBe("unknown");
    expect(normalizeInvokeError(undefined).code).toBe("unknown");
  });
});
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cd apps/codex-assistant-manager && npm run test
```
Expected: `4 passed`，退出码 0。

- [ ] **Step 5: 跑 tsc 确认类型检查通过**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 6: 提交**

```bash
git add apps/codex-assistant-manager/src/lib/
git commit -m "feat(manager): add text constants and invoke wrapper with normalized errors"
```

---

## Task 3: launcherMachine 纯状态机 + 5 态单测

**Files:**
- Create: `apps/codex-assistant-manager/src/state/launcherMachine.ts`
- Create: `apps/codex-assistant-manager/src/state/launcherMachine.test.ts`

- [ ] **Step 1: 写状态机 `launcherMachine.ts`**

```ts
// apps/codex-assistant-manager/src/state/launcherMachine.ts
export type ProbeResult = {
  watcherInstalled: boolean;
  watcherEnabled: boolean;
  relayApplied: boolean;
  hasAccount: boolean;
};

export type LauncherState =
  | { kind: "preparing" }
  | { kind: "ready" }
  | { kind: "launching" }
  | { kind: "need_account" }
  | { kind: "error"; message: string };

export type LauncherEvent =
  | { type: "probe_done"; result: ProbeResult }
  | { type: "prepare_start" }
  | { type: "prepare_done" }
  | { type: "prepare_failed"; message: string }
  | { type: "launch_click" }
  | { type: "launch_done" }
  | { type: "launch_failed"; message: string }
  | { type: "retry" };

export const initialLauncherState: LauncherState = { kind: "preparing" };

export function deriveStateFromProbe(result: ProbeResult): LauncherState {
  if (!result.hasAccount) return { kind: "need_account" };
  if (!result.watcherInstalled || !result.watcherEnabled || !result.relayApplied) {
    return { kind: "preparing" };
  }
  return { kind: "ready" };
}

export function launcherReducer(state: LauncherState, event: LauncherEvent): LauncherState {
  switch (event.type) {
    case "probe_done":
      return deriveStateFromProbe(event.result);
    case "prepare_start":
      return { kind: "preparing" };
    case "prepare_done":
      return { kind: "ready" };
    case "prepare_failed":
      return { kind: "error", message: event.message };
    case "launch_click":
      return state.kind === "ready" ? { kind: "launching" } : state;
    case "launch_done":
      return state.kind === "launching" ? { kind: "ready" } : state;
    case "launch_failed":
      return { kind: "error", message: event.message };
    case "retry":
      return state.kind === "error" ? { kind: "preparing" } : state;
  }
}
```

- [ ] **Step 2: 写覆盖 5 态切换的失败测试**

```ts
// apps/codex-assistant-manager/src/state/launcherMachine.test.ts
import { describe, expect, it } from "vitest";
import {
  deriveStateFromProbe,
  initialLauncherState,
  launcherReducer,
  type ProbeResult,
} from "./launcherMachine";

const fullProbe: ProbeResult = {
  watcherInstalled: true,
  watcherEnabled: true,
  relayApplied: true,
  hasAccount: true,
};

describe("deriveStateFromProbe", () => {
  it("returns ready when everything is set", () => {
    expect(deriveStateFromProbe(fullProbe)).toEqual({ kind: "ready" });
  });
  it("returns need_account when account missing", () => {
    expect(deriveStateFromProbe({ ...fullProbe, hasAccount: false })).toEqual({
      kind: "need_account",
    });
  });
  it("returns preparing when watcher uninstalled", () => {
    expect(deriveStateFromProbe({ ...fullProbe, watcherInstalled: false })).toEqual({
      kind: "preparing",
    });
  });
  it("returns preparing when relay not applied", () => {
    expect(deriveStateFromProbe({ ...fullProbe, relayApplied: false })).toEqual({
      kind: "preparing",
    });
  });
});

describe("launcherReducer", () => {
  it("starts in preparing", () => {
    expect(initialLauncherState).toEqual({ kind: "preparing" });
  });

  it("ready -> launching on launch_click", () => {
    expect(launcherReducer({ kind: "ready" }, { type: "launch_click" })).toEqual({
      kind: "launching",
    });
  });

  it("ignores launch_click outside ready", () => {
    expect(launcherReducer({ kind: "preparing" }, { type: "launch_click" })).toEqual({
      kind: "preparing",
    });
  });

  it("launching -> ready on launch_done", () => {
    expect(launcherReducer({ kind: "launching" }, { type: "launch_done" })).toEqual({
      kind: "ready",
    });
  });

  it("any -> error on launch_failed", () => {
    expect(
      launcherReducer({ kind: "launching" }, { type: "launch_failed", message: "boom" }),
    ).toEqual({ kind: "error", message: "boom" });
  });

  it("error -> preparing on retry", () => {
    expect(
      launcherReducer({ kind: "error", message: "x" }, { type: "retry" }),
    ).toEqual({ kind: "preparing" });
  });

  it("prepare_failed sets error with message", () => {
    expect(
      launcherReducer({ kind: "preparing" }, { type: "prepare_failed", message: "nope" }),
    ).toEqual({ kind: "error", message: "nope" });
  });

  it("probe_done updates from probe result", () => {
    expect(
      launcherReducer({ kind: "preparing" }, { type: "probe_done", result: fullProbe }),
    ).toEqual({ kind: "ready" });
  });
});
```

- [ ] **Step 3: 跑测试**

```bash
cd apps/codex-assistant-manager && npm run test
```
Expected: `13 passed`（4 + 9），退出码 0。

- [ ] **Step 4: 跑 tsc**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 5: 提交**

```bash
git add apps/codex-assistant-manager/src/state/launcherMachine.ts apps/codex-assistant-manager/src/state/launcherMachine.test.ts
git commit -m "feat(manager): add launcher state machine with 5-state reducer"
```

---

## Task 4: useBackend / useLauncherMachine / useUpdateProbe hooks

**Files:**
- Create: `apps/codex-assistant-manager/src/state/useBackend.ts`
- Create: `apps/codex-assistant-manager/src/state/useLauncherMachine.ts`
- Create: `apps/codex-assistant-manager/src/state/useUpdateProbe.ts`

- [ ] **Step 1: 写 useBackend hook**

```ts
// apps/codex-assistant-manager/src/state/useBackend.ts
import { useCallback, useEffect, useState } from "react";
import { callSafe, type NormalizedError } from "@/lib/invoke";
import type { ProbeResult } from "./launcherMachine";

export type OverviewLite = {
  hasAccount: boolean;
  appPath: string | null;
  debugPort: number;
  helperPort: number;
};

export type SettingsLite = {
  hasApiKey: boolean;
  apiKey: string;
  baseUrl: string;
};

async function loadProbe(): Promise<
  | { probe: ProbeResult; overview: OverviewLite; settings: SettingsLite }
  | NormalizedError
> {
  const overview = await callSafe<Record<string, unknown>>("load_overview");
  if (!overview.ok) return overview.error;
  const watcher = await callSafe<Record<string, unknown>>("load_watcher_state");
  if (!watcher.ok) return watcher.error;
  const relay = await callSafe<Record<string, unknown>>("relay_status");
  if (!relay.ok) return relay.error;
  const settings = await callSafe<Record<string, unknown>>("load_settings");
  if (!settings.ok) return settings.error;

  const ov = overview.data as { has_account?: boolean; app_path?: string|null; debug_port?: number; helper_port?: number };
  const wa = watcher.data as { installed?: boolean; enabled?: boolean };
  const re = relay.data as { applied?: boolean };
  const stRaw = settings.data as { settings?: Record<string, unknown> };
  const st = (stRaw.settings ?? {}) as { officialMixApiKey?: string|null; officialMixBaseUrl?: string|null };

  const apiKey = (st.officialMixApiKey ?? "").trim();
  const baseUrl = (st.officialMixBaseUrl ?? "").trim();

  return {
    overview: {
      hasAccount: !!ov.has_account,
      appPath: ov.app_path ?? null,
      debugPort: ov.debug_port ?? 9229,
      helperPort: ov.helper_port ?? 57321,
    },
    probe: {
      watcherInstalled: !!wa.installed,
      watcherEnabled: !!wa.enabled,
      relayApplied: !!re.applied,
      hasAccount: !!ov.has_account,
    },
    settings: { hasApiKey: apiKey.length > 0, apiKey, baseUrl },
  };
}

export function useBackend() {
  const [overview, setOverview] = useState<OverviewLite | null>(null);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [settings, setSettings] = useState<SettingsLite | null>(null);
  const [error, setError] = useState<NormalizedError | null>(null);

  const refresh = useCallback(async () => {
    const result = await loadProbe();
    if ("probe" in result) {
      setOverview(result.overview);
      setProbe(result.probe);
      setSettings(result.settings);
      setError(null);
    } else {
      setError(result);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { overview, probe, settings, error, refresh };
}
```

> 字段名约定：渲染层用 camelCase；Tauri 返回的 JSON 字段名以 commands.rs 实际产出为准（部分 endpoint 已 hand-roll 成 camelCase，例如 update 链路的 `assetSha256`）。所有 normalize 在本文件单点完成；若运行时发现字段不匹配，**只改这里的 `as` 段口径**。

- [ ] **Step 2: 写 useLauncherMachine hook**

```ts
// apps/codex-assistant-manager/src/state/useLauncherMachine.ts
import { useCallback, useEffect, useReducer } from "react";
import { callSafe } from "@/lib/invoke";
import {
  initialLauncherState,
  launcherReducer,
  type LauncherState,
  type ProbeResult,
} from "./launcherMachine";

export type RelayKind = "chatgpt" | "apiKey" | "none";

export type LauncherDeps = {
  probe: ProbeResult | null;
  relayKind: RelayKind;
  launchArgs: { appPath: string | null; debugPort: number; helperPort: number };
  onAfterLaunch: () => Promise<void> | void;
};

export function useLauncherMachine(deps: LauncherDeps): {
  state: LauncherState;
  launch: () => Promise<void>;
  retry: () => Promise<void>;
} {
  const [state, dispatch] = useReducer(launcherReducer, initialLauncherState);

  const prepare = useCallback(async () => {
    dispatch({ type: "prepare_start" });
    if (!deps.probe?.watcherInstalled) {
      const r = await callSafe("install_watcher");
      if (!r.ok) return dispatch({ type: "prepare_failed", message: r.error.message });
    }
    if (!deps.probe?.watcherEnabled) {
      const r = await callSafe("enable_watcher");
      if (!r.ok) return dispatch({ type: "prepare_failed", message: r.error.message });
    }
    if (!deps.probe?.relayApplied) {
      const cmd =
        deps.relayKind === "apiKey" ? "apply_pure_api_injection"
          : deps.relayKind === "chatgpt" ? "apply_relay_injection"
          : "clear_relay_injection";
      const r = await callSafe(cmd);
      if (!r.ok) return dispatch({ type: "prepare_failed", message: r.error.message });
    }
    dispatch({ type: "prepare_done" });
  }, [deps.probe, deps.relayKind]);

  useEffect(() => {
    if (!deps.probe) return;
    dispatch({ type: "probe_done", result: deps.probe });
    if (deps.probe.hasAccount &&
      (!deps.probe.watcherInstalled || !deps.probe.watcherEnabled || !deps.probe.relayApplied)) {
      void prepare();
    }
  }, [deps.probe, prepare]);

  const launch = useCallback(async () => {
    dispatch({ type: "launch_click" });
    let r = await callSafe<Record<string, unknown>>("launch_codex_assistant", {
      request: deps.launchArgs,
    });
    // spec §8 端口冲突自动修复：失败时尝试一次 repair_backend 后重试
    if (!r.ok) {
      const repair = await callSafe("repair_backend");
      if (repair.ok) {
        r = await callSafe<Record<string, unknown>>("launch_codex_assistant", {
          request: deps.launchArgs,
        });
      }
    }
    if (!r.ok) {
      dispatch({ type: "launch_failed", message: r.error.message });
      return;
    }
    dispatch({ type: "launch_done" });
    await deps.onAfterLaunch();
  }, [deps.launchArgs, deps.onAfterLaunch]);

  const retry = useCallback(async () => {
    dispatch({ type: "retry" });
    await prepare();
  }, [prepare]);

  return { state, launch, retry };
}
```

- [ ] **Step 3: 写 useUpdateProbe hook**

```ts
// apps/codex-assistant-manager/src/state/useUpdateProbe.ts
import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

export type UpdateInfo = {
  available: boolean;
  latestVersion: string | null;
  assetUrl: string | null;
  assetSha256: string | null;
  assetName: string | null;
};

export function useUpdateProbe(delayMs = 5000): UpdateInfo | null {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  useEffect(() => {
    const timer = setTimeout(async () => {
      const r = await callSafe<Record<string, unknown>>("check_update");
      if (!r.ok) return;
      const d = r.data as {
        update_available?: boolean;
        latest_version?: string | null;
        assetUrl?: string | null;
        assetName?: string | null;
        assetSha256?: string | null;
      };
      setInfo({
        available: !!d.update_available,
        latestVersion: d.latest_version ?? null,
        assetUrl: d.assetUrl ?? null,
        assetName: d.assetName ?? null,
        assetSha256: d.assetSha256 ?? null,
      });
    }, delayMs);
    return () => clearTimeout(timer);
  }, [delayMs]);
  return info;
}
```

- [ ] **Step 4: 跑 tsc 确认类型通过**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 5: 跑 vitest 确认没回归**

```bash
cd apps/codex-assistant-manager && npm run test
```
Expected: 13 passed。

- [ ] **Step 6: 提交**

```bash
git add apps/codex-assistant-manager/src/state/
git commit -m "feat(manager): add useBackend / useLauncherMachine / useUpdateProbe hooks"
```

---

## Task 5: 通用 Drawer + 5 个原子组件

**Files:**
- Create: `apps/codex-assistant-manager/src/components/Drawer.tsx`
- Create: `apps/codex-assistant-manager/src/components/LauncherButton.tsx`
- Create: `apps/codex-assistant-manager/src/components/CapabilityChips.tsx`
- Create: `apps/codex-assistant-manager/src/components/UpdateBanner.tsx`
- Create: `apps/codex-assistant-manager/src/components/AccountStatusCard.tsx`

- [ ] **Step 1: Drawer.tsx**

```tsx
// apps/codex-assistant-manager/src/components/Drawer.tsx
import { useEffect, type ReactNode } from "react";
import { X } from "lucide-react";

export function Drawer({
  open,
  title,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex">
      <div className="flex-1 bg-black/40" onClick={onClose} />
      <aside className="w-[420px] max-w-[90vw] h-full bg-background border-l border-border shadow-xl flex flex-col">
        <header className="flex items-center justify-between px-5 py-4 border-b border-border">
          <h2 className="text-lg font-medium">{title}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-muted">
            <X className="size-5" />
          </button>
        </header>
        <div className="flex-1 overflow-y-auto px-5 py-4">{children}</div>
      </aside>
    </div>
  );
}
```

- [ ] **Step 2: LauncherButton.tsx**

```tsx
// apps/codex-assistant-manager/src/components/LauncherButton.tsx
import { Loader2, Rocket, AlertTriangle, ArrowRight } from "lucide-react";
import type { LauncherState } from "@/state/launcherMachine";
import { TEXT } from "@/lib/text";

export function LauncherButton({
  state,
  onLaunch,
  onRetry,
}: {
  state: LauncherState;
  onLaunch: () => void;
  onRetry: () => void;
}) {
  const base =
    "w-[320px] h-[120px] rounded-2xl text-xl font-semibold flex flex-col items-center justify-center gap-2 transition";
  switch (state.kind) {
    case "ready":
      return (
        <button onClick={onLaunch} className={`${base} bg-primary text-primary-foreground hover:opacity-90`}>
          <span className="flex items-center gap-2"><Rocket className="size-6" /> {TEXT.launcher.ready}</span>
          <span className="text-sm font-normal opacity-80">{TEXT.launcher.readyHint}</span>
        </button>
      );
    case "launching":
    case "preparing":
      return (
        <button disabled className={`${base} bg-primary/80 text-primary-foreground cursor-not-allowed`}>
          <span className="flex items-center gap-2"><Loader2 className="size-6 animate-spin" />
            {state.kind === "launching" ? TEXT.launcher.launching : TEXT.launcher.preparing}
          </span>
        </button>
      );
    case "need_account":
      return (
        <button onClick={onLaunch} className={`${base} border border-border bg-background hover:bg-muted`}>
          <span className="flex items-center gap-2"><ArrowRight className="size-6" /> {TEXT.launcher.needAccount}</span>
        </button>
      );
    case "error":
      return (
        <button onClick={onRetry} className={`${base} border border-destructive text-destructive bg-background hover:bg-destructive/10`}>
          <span className="flex items-center gap-2"><AlertTriangle className="size-6" /> {TEXT.launcher.errorPrefix}</span>
          <span className="text-sm font-normal opacity-80">{state.message}</span>
        </button>
      );
  }
}
```

- [ ] **Step 3: CapabilityChips.tsx**

```tsx
// apps/codex-assistant-manager/src/components/CapabilityChips.tsx
import { Check } from "lucide-react";
import { TEXT } from "@/lib/text";

const items = [
  TEXT.capabilities.plugins,
  TEXT.capabilities.deleteChats,
  TEXT.capabilities.exportMd,
  TEXT.capabilities.autoUpdate,
];

export function CapabilityChips() {
  return (
    <div className="flex flex-wrap items-center justify-center gap-3 text-sm text-muted-foreground">
      {items.map((label) => (
        <span key={label} className="inline-flex items-center gap-1">
          <Check className="size-4 text-primary" />
          {label}
        </span>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: UpdateBanner.tsx**

```tsx
// apps/codex-assistant-manager/src/components/UpdateBanner.tsx
import { TEXT } from "@/lib/text";
import type { UpdateInfo } from "@/state/useUpdateProbe";

export function UpdateBanner({
  info,
  onUpdate,
}: {
  info: UpdateInfo | null;
  onUpdate: () => void;
}) {
  if (!info?.available || !info.latestVersion) return null;
  return (
    <p className="text-sm text-muted-foreground">
      {TEXT.update.available(info.latestVersion)}{"  "}
      <button onClick={onUpdate} className="underline text-primary hover:opacity-80">
        {TEXT.update.cta}
      </button>
    </p>
  );
}
```

- [ ] **Step 5: AccountStatusCard.tsx**

```tsx
// apps/codex-assistant-manager/src/components/AccountStatusCard.tsx
import { UserCircle2 } from "lucide-react";
import { TEXT } from "@/lib/text";
import type { RelayKind } from "@/state/useLauncherMachine";

export function AccountStatusCard({
  relayKind,
  onClick,
}: {
  relayKind: RelayKind;
  onClick: () => void;
}) {
  const label =
    relayKind === "apiKey" ? TEXT.account.apiKey
      : relayKind === "chatgpt" ? TEXT.account.chatgpt
      : "未配置";
  return (
    <button
      onClick={onClick}
      className="inline-flex items-center gap-2 px-4 py-2 rounded-xl border border-border hover:bg-muted text-sm"
    >
      <UserCircle2 className="size-5" />
      <span className="opacity-80">{TEXT.account.title}</span>
      <span className="font-medium">{label}</span>
    </button>
  );
}
```

- [ ] **Step 6: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 7: 提交**

```bash
git add apps/codex-assistant-manager/src/components/Drawer.tsx apps/codex-assistant-manager/src/components/LauncherButton.tsx apps/codex-assistant-manager/src/components/CapabilityChips.tsx apps/codex-assistant-manager/src/components/UpdateBanner.tsx apps/codex-assistant-manager/src/components/AccountStatusCard.tsx
git commit -m "feat(manager): add launcher / drawer / chips / banner / account-status components"
```

---

## Task 6: Home 屏

**Files:**
- Create: `apps/codex-assistant-manager/src/screens/Home.tsx`

- [ ] **Step 1: 写 Home.tsx**

```tsx
// apps/codex-assistant-manager/src/screens/Home.tsx
import { Settings } from "lucide-react";
import { TEXT } from "@/lib/text";
import { LauncherButton } from "@/components/LauncherButton";
import { CapabilityChips } from "@/components/CapabilityChips";
import { UpdateBanner } from "@/components/UpdateBanner";
import { AccountStatusCard } from "@/components/AccountStatusCard";
import type { LauncherState } from "@/state/launcherMachine";
import type { RelayKind } from "@/state/useLauncherMachine";
import type { UpdateInfo } from "@/state/useUpdateProbe";

export function Home({
  state,
  relayKind,
  updateInfo,
  onLaunch,
  onRetry,
  onOpenAccount,
  onOpenMore,
  onOpenUpdate,
}: {
  state: LauncherState;
  relayKind: RelayKind;
  updateInfo: UpdateInfo | null;
  onLaunch: () => void;
  onRetry: () => void;
  onOpenAccount: () => void;
  onOpenMore: () => void;
  onOpenUpdate: () => void;
}) {
  return (
    <main className="min-h-screen flex flex-col">
      <header className="flex items-center justify-between px-6 py-4">
        <h1 className="text-base font-medium">{TEXT.appName}</h1>
        <button
          onClick={onOpenMore}
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <Settings className="size-4" /> {TEXT.more.title}
        </button>
      </header>

      <section className="flex-1 flex flex-col items-center justify-center gap-6 px-6">
        <LauncherButton state={state} onLaunch={onLaunch} onRetry={onRetry} />
        <CapabilityChips />
        <UpdateBanner info={updateInfo} onUpdate={onOpenUpdate} />
      </section>

      <footer className="flex justify-end px-6 py-4">
        <AccountStatusCard relayKind={relayKind} onClick={onOpenAccount} />
      </footer>
    </main>
  );
}
```

- [ ] **Step 2: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 3: 提交**

```bash
git add apps/codex-assistant-manager/src/screens/Home.tsx
git commit -m "feat(manager): add Home screen composition"
```

---

## Task 7: AccountDrawer（中转 / API Key 切换）

**Files:**
- Create: `apps/codex-assistant-manager/src/drawers/AccountDrawer.tsx`

- [ ] **Step 1: 写 AccountDrawer.tsx**

```tsx
// apps/codex-assistant-manager/src/drawers/AccountDrawer.tsx
import { useState } from "react";
import { Drawer } from "@/components/Drawer";
import { TEXT } from "@/lib/text";
import { callSafe } from "@/lib/invoke";
import type { RelayKind } from "@/state/useLauncherMachine";

export function AccountDrawer({
  open,
  onClose,
  current,
  onApplied,
}: {
  open: boolean;
  onClose: () => void;
  current: RelayKind;
  onApplied: () => Promise<void>;
}) {
  const [kind, setKind] = useState<RelayKind>(current);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openLogin = async () => {
    setBusy(true); setError(null);
    const r = await callSafe("apply_relay_injection");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    await onApplied();
    onClose();
  };

  const saveApiKey = async () => {
    setBusy(true); setError(null);
    const trimmed = apiKey.trim();
    if (!trimmed) { setBusy(false); setError("API Key 不能为空"); return; }
    const save = await callSafe("save_settings", {
      settings: { officialMixApiKey: trimmed, officialMixBaseUrl: baseUrl.trim() || null },
    });
    if (!save.ok) { setBusy(false); setError(save.error.message); return; }
    const apply = await callSafe("apply_pure_api_injection");
    setBusy(false);
    if (!apply.ok) { setError(apply.error.message); return; }
    await onApplied();
    onClose();
  };

  return (
    <Drawer open={open} title={TEXT.account.title} onClose={onClose}>
      <div className="space-y-6">
        <label className="flex items-start gap-3">
          <input type="radio" checked={kind === "chatgpt"} onChange={() => setKind("chatgpt")} className="mt-1" />
          <div className="flex-1">
            <div>{TEXT.account.chatgpt}</div>
            {kind === "chatgpt" && (
              <button onClick={openLogin} disabled={busy} className="mt-2 px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm">
                {TEXT.account.openLogin}
              </button>
            )}
          </div>
        </label>

        <label className="flex items-start gap-3">
          <input type="radio" checked={kind === "apiKey"} onChange={() => setKind("apiKey")} className="mt-1" />
          <div className="flex-1 space-y-2">
            <div>{TEXT.account.apiKey}</div>
            {kind === "apiKey" && (
              <>
                <input
                  value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..."
                  className="w-full px-2 py-1 border border-border rounded bg-background"
                />
                <input
                  value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="Base URL（可选）"
                  className="w-full px-2 py-1 border border-border rounded bg-background"
                />
                <button onClick={saveApiKey} disabled={busy} className="px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm">
                  {TEXT.account.saveSwitch}
                </button>
              </>
            )}
          </div>
        </label>

        {error && <p className="text-sm text-destructive">{error}</p>}
        <p className="text-xs text-muted-foreground">
          {TEXT.account.current}：{current === "apiKey" ? TEXT.account.apiKey : current === "chatgpt" ? TEXT.account.chatgpt : "未配置"}
        </p>
      </div>
    </Drawer>
  );
}
```

> 字段名 `officialMixApiKey` / `officialMixBaseUrl` 已与 `BackendSettings`（`crates/codex-assistant-core/src/settings.rs`）一致。若 tsc 报字段不匹配，按真实 settings 字段名修正——保持单点修改即可。

- [ ] **Step 2: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 3: 提交**

```bash
git add apps/codex-assistant-manager/src/drawers/AccountDrawer.tsx
git commit -m "feat(manager): add AccountDrawer with ChatGPT / API Key two-option switch"
```

---

## Task 8: panels 1/2 — Scripts / Providers / EntryPoints

**Files:**
- Create: `apps/codex-assistant-manager/src/panels/ScriptsPanel.tsx`
- Create: `apps/codex-assistant-manager/src/panels/ProvidersPanel.tsx`
- Create: `apps/codex-assistant-manager/src/panels/EntryPointsPanel.tsx`

- [ ] **Step 1: ScriptsPanel.tsx（脚本市场，最小版）**

```tsx
// apps/codex-assistant-manager/src/panels/ScriptsPanel.tsx
import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

type MarketItem = { id: string; name: string; description?: string; version: string; author?: string };
type Payload = { market?: { scripts: MarketItem[] }; installed?: Record<string, { enabled: boolean }> };

export function ScriptsPanel() {
  const [data, setData] = useState<Payload | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setBusy(true); setError(null);
    const r = await callSafe<Payload>("refresh_script_market");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setData(r.data);
  };

  useEffect(() => { void refresh(); }, []);

  const install = async (id: string) => {
    setBusy(true); setError(null);
    const r = await callSafe<Payload>("install_market_script", { id });
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setData(r.data);
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">脚本市场</h3>
        <button onClick={refresh} disabled={busy} className="text-xs underline text-primary">刷新</button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
      <ul className="space-y-2">
        {(data?.market?.scripts ?? []).map((item) => {
          const installed = !!data?.installed?.[item.id];
          return (
            <li key={item.id} className="flex items-center justify-between border border-border rounded px-3 py-2">
              <div>
                <div className="text-sm font-medium">{item.name} <span className="text-xs text-muted-foreground">v{item.version}</span></div>
                {item.description && <div className="text-xs text-muted-foreground">{item.description}</div>}
              </div>
              <button onClick={() => install(item.id)} disabled={busy || installed} className="text-xs px-2 py-1 rounded bg-primary text-primary-foreground disabled:opacity-50">
                {installed ? "已安装" : "安装"}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
```

- [ ] **Step 2: ProvidersPanel.tsx**

```tsx
// apps/codex-assistant-manager/src/panels/ProvidersPanel.tsx
import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function ProvidersPanel() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const importNow = async () => {
    setBusy(true); setError(null); setMsg(null);
    const r = await callSafe<{ message?: string }>("import_ccs_providers");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setMsg(r.data.message ?? "已导入");
  };

  const sync = async () => {
    setBusy(true); setError(null); setMsg(null);
    const r = await callSafe<{ message?: string }>("sync_providers_now");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setMsg("同步完成");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">服务源（CCS）</h3>
      <div className="flex gap-2">
        <button onClick={importNow} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">导入 CCS 配置</button>
        <button onClick={sync} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">立即同步</button>
      </div>
      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 3: EntryPointsPanel.tsx**

```tsx
// apps/codex-assistant-manager/src/panels/EntryPointsPanel.tsx
import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function EntryPointsPanel() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const wrap = (fn: () => Promise<{ ok: boolean; error?: { message: string }; data?: { message?: string } }>) =>
    async () => {
      setBusy(true); setError(null); setMsg(null);
      const r = await fn();
      setBusy(false);
      if (!r.ok) { setError(r.error?.message ?? "出错了"); return; }
      setMsg(r.data?.message ?? "完成");
    };

  const install = wrap(() => callSafe("install_entrypoints"));
  const repair = wrap(() => callSafe("repair_shortcuts"));
  const uninstall = wrap(() => callSafe("uninstall_entrypoints", { options: { keepData: true } }));

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">桌面快捷方式</h3>
      <div className="flex flex-wrap gap-2">
        <button onClick={install} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">安装</button>
        <button onClick={repair} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">修复</button>
        <button onClick={uninstall} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">卸载（保留数据）</button>
      </div>
      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 4: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 5: 提交**

```bash
git add apps/codex-assistant-manager/src/panels/ScriptsPanel.tsx apps/codex-assistant-manager/src/panels/ProvidersPanel.tsx apps/codex-assistant-manager/src/panels/EntryPointsPanel.tsx
git commit -m "feat(manager): add Scripts / Providers / EntryPoints panels"
```

---

## Task 9: panels 2/2 — Diagnostics / RelayAdvanced / About

**Files:**
- Create: `apps/codex-assistant-manager/src/panels/DiagnosticsPanel.tsx`
- Create: `apps/codex-assistant-manager/src/panels/RelayAdvancedPanel.tsx`
- Create: `apps/codex-assistant-manager/src/panels/AboutPanel.tsx`

- [ ] **Step 1: DiagnosticsPanel.tsx**

```tsx
// apps/codex-assistant-manager/src/panels/DiagnosticsPanel.tsx
import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function DiagnosticsPanel() {
  const [busy, setBusy] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const exportPack = async () => {
    setBusy(true); setError(null); setPath(null);
    const r = await callSafe<{ path?: string }>("copy_diagnostics");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setPath(r.data.path ?? null);
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">反馈包</h3>
      <button onClick={exportPack} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">
        导出反馈包到桌面
      </button>
      {path && <p className="text-xs text-muted-foreground">已导出：{path}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 2: RelayAdvancedPanel.tsx**

```tsx
// apps/codex-assistant-manager/src/panels/RelayAdvancedPanel.tsx
import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

type Files = { configContents: string; authContents: string };

export function RelayAdvancedPanel() {
  const [files, setFiles] = useState<Files>({ configContents: "", authContents: "" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const r = await callSafe<Files>("read_relay_files");
      if (r.ok) setFiles(r.data);
    })();
  }, []);

  const save = async (target: "config" | "auth") => {
    setBusy(true); setError(null); setMsg(null);
    const contents = target === "config" ? files.configContents : files.authContents;
    const r = await callSafe<Files>("save_relay_file", { request: { target, contents } });
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setFiles(r.data); setMsg("已保存");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">中转配置文件</h3>
      <p className="text-xs text-muted-foreground">高级选项；除非你知道在改什么，否则不要动。</p>

      <label className="text-xs">config.toml</label>
      <textarea
        value={files.configContents}
        onChange={(e) => setFiles({ ...files, configContents: e.target.value })}
        className="w-full h-32 px-2 py-1 border border-border rounded bg-background font-mono text-xs"
      />
      <button onClick={() => save("config")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">保存 config</button>

      <label className="text-xs">auth.json</label>
      <textarea
        value={files.authContents}
        onChange={(e) => setFiles({ ...files, authContents: e.target.value })}
        className="w-full h-32 px-2 py-1 border border-border rounded bg-background font-mono text-xs"
      />
      <button onClick={() => save("auth")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">保存 auth</button>

      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 3: AboutPanel.tsx**

```tsx
// apps/codex-assistant-manager/src/panels/AboutPanel.tsx
import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

export function AboutPanel() {
  const [version, setVersion] = useState<string>("...");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const r = await callSafe<{ version: string }>("backend_version");
      if (r.ok) setVersion(r.data.version);
    })();
  }, []);

  const reset = async () => {
    if (!confirm("确认重置所有设置？此操作不可撤销。")) return;
    setBusy(true); setError(null);
    const r = await callSafe("reset_settings");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    alert("已重置，请重启应用");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">关于</h3>
      <p className="text-xs text-muted-foreground">版本：{version}</p>
      <button onClick={reset} disabled={busy} className="text-xs px-2 py-1 rounded border border-destructive text-destructive">
        重置所有设置
      </button>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 4: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 5: 提交**

```bash
git add apps/codex-assistant-manager/src/panels/DiagnosticsPanel.tsx apps/codex-assistant-manager/src/panels/RelayAdvancedPanel.tsx apps/codex-assistant-manager/src/panels/AboutPanel.tsx
git commit -m "feat(manager): add Diagnostics / RelayAdvanced / About panels"
```

---

## Task 10: MoreDrawer 聚合 6 个 panels

**Files:**
- Create: `apps/codex-assistant-manager/src/drawers/MoreDrawer.tsx`

- [ ] **Step 1: 写 MoreDrawer.tsx**

```tsx
// apps/codex-assistant-manager/src/drawers/MoreDrawer.tsx
import { type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { Drawer } from "@/components/Drawer";
import { TEXT } from "@/lib/text";
import { ScriptsPanel } from "@/panels/ScriptsPanel";
import { ProvidersPanel } from "@/panels/ProvidersPanel";
import { EntryPointsPanel } from "@/panels/EntryPointsPanel";
import { DiagnosticsPanel } from "@/panels/DiagnosticsPanel";
import { RelayAdvancedPanel } from "@/panels/RelayAdvancedPanel";
import { AboutPanel } from "@/panels/AboutPanel";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <details className="group border border-border rounded">
      <summary className="cursor-pointer list-none px-3 py-2 flex items-center justify-between text-sm">
        {title}
        <ChevronDown className="size-4 transition group-open:rotate-180" />
      </summary>
      <div className="px-3 py-3 border-t border-border">{children}</div>
    </details>
  );
}

export function MoreDrawer({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Drawer open={open} title={TEXT.more.title} onClose={onClose}>
      <div className="space-y-3">
        <Section title={TEXT.more.sections.scripts}><ScriptsPanel /></Section>
        <Section title={TEXT.more.sections.providers}><ProvidersPanel /></Section>
        <Section title={TEXT.more.sections.entryPoints}><EntryPointsPanel /></Section>
        <Section title={TEXT.more.sections.diagnostics}><DiagnosticsPanel /></Section>
        <Section title={TEXT.more.sections.relayAdvanced}><RelayAdvancedPanel /></Section>
        <Section title={TEXT.more.sections.about}><AboutPanel /></Section>
      </div>
    </Drawer>
  );
}
```

- [ ] **Step 2: tsc 验证**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。

- [ ] **Step 3: 提交**

```bash
git add apps/codex-assistant-manager/src/drawers/MoreDrawer.tsx
git commit -m "feat(manager): add MoreDrawer aggregating six collapsible panels"
```

---

## Task 11: 替换 App.tsx 编排一切 + 接通自动更新

**Files:**
- Replace contents of: `apps/codex-assistant-manager/src/App.tsx`

- [ ] **Step 1: 用新 App.tsx 完整替换旧 2952 行**

```tsx
// apps/codex-assistant-manager/src/App.tsx
import { useCallback, useMemo, useState } from "react";
import { Home } from "@/screens/Home";
import { AccountDrawer } from "@/drawers/AccountDrawer";
import { MoreDrawer } from "@/drawers/MoreDrawer";
import { useBackend } from "@/state/useBackend";
import { useLauncherMachine, type RelayKind } from "@/state/useLauncherMachine";
import { useUpdateProbe } from "@/state/useUpdateProbe";
import { callSafe } from "@/lib/invoke";
import { TEXT } from "@/lib/text";

function inferRelayKind(applied: boolean, hasApiKey: boolean): RelayKind {
  if (!applied) return "none";
  return hasApiKey ? "apiKey" : "chatgpt";
}

export function App() {
  const { overview, probe, settings, refresh } = useBackend();
  const [accountOpen, setAccountOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [updateBusy, setUpdateBusy] = useState(false);

  const relayKind: RelayKind = useMemo(
    () => inferRelayKind(!!probe?.relayApplied, !!settings?.hasApiKey),
    [probe?.relayApplied, settings?.hasApiKey],
  );

  const launchArgs = useMemo(
    () => ({
      appPath: overview?.appPath ?? null,
      debugPort: overview?.debugPort ?? 9229,
      helperPort: overview?.helperPort ?? 57321,
    }),
    [overview?.appPath, overview?.debugPort, overview?.helperPort],
  );

  const onAfterLaunch = useCallback(async () => { await refresh(); }, [refresh]);

  const { state, launch, retry } = useLauncherMachine({
    probe,
    relayKind,
    launchArgs,
    onAfterLaunch,
  });

  const updateInfo = useUpdateProbe(5000);

  const onOpenUpdate = useCallback(async () => {
    if (!updateInfo?.available) return;
    setUpdateBusy(true);
    const release = {
      version: updateInfo.latestVersion ?? "",
      url: "",
      body: "",
      asset_name: updateInfo.assetName,
      asset_url: updateInfo.assetUrl,
      asset_sha256: updateInfo.assetSha256,
    };
    const r = await callSafe("perform_update", { release });
    setUpdateBusy(false);
    if (!r.ok) {
      alert(`${TEXT.update.failedTitle}\n\n${r.error.message}`);
    }
  }, [updateInfo]);

  return (
    <>
      <Home
        state={state}
        relayKind={relayKind}
        updateInfo={updateInfo}
        onLaunch={launch}
        onRetry={retry}
        onOpenAccount={() => setAccountOpen(true)}
        onOpenMore={() => setMoreOpen(true)}
        onOpenUpdate={onOpenUpdate}
      />
      <AccountDrawer
        open={accountOpen}
        onClose={() => setAccountOpen(false)}
        current={relayKind}
        onApplied={refresh}
      />
      <MoreDrawer open={moreOpen} onClose={() => setMoreOpen(false)} />
      {updateBusy && (
        <div className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center">
          <div className="bg-background border border-border rounded p-6 text-sm">正在下载并校验更新…</div>
        </div>
      )}
    </>
  );
}
```

- [ ] **Step 2: 跑 tsc——这是大检查点**

```bash
cd apps/codex-assistant-manager && npm run check
```
Expected: 0 错误。如果有，按报错修正（**只可能是 normalize 字段名不匹配，所有调整点都集中在 `useBackend.ts` 和 `useUpdateProbe.ts` 里**）。

- [ ] **Step 3: 跑 vitest 确保状态机测试仍过**

```bash
cd apps/codex-assistant-manager && npm run test
```
Expected: 13 passed。

- [ ] **Step 4: 跑 vite build 验证打包**

```bash
cd apps/codex-assistant-manager && npm run vite:build
```
Expected: 成功，无错误。

- [ ] **Step 5: 提交**

```bash
git add apps/codex-assistant-manager/src/App.tsx
git commit -m "feat(manager): replace App.tsx with launcher-first composition"
```

---

## Task 12: 手测脚本

**Files:**
- Create: `docs/superpowers/specs/2026-05-23-beginner-ux-redesign-manual-smoke.md`

- [ ] **Step 1: 写手测脚本**

```markdown
# Manual Smoke Test — Beginner UX Redesign

8 个场景，每个标 [PASS] / [FAIL]。失败要附截图或错误文本。

## 1. 首次启动（冷装机）
- 进入应用，首屏只有一个巨型按钮 + 4 胶囊。
- 按钮自动从「准备增强…」过渡到「打开 ChatGPT」。
- 点击按钮，ChatGPT 在外部浏览器/Tauri 窗内打开，注入脚本生效（出现"删对话"按钮）。

## 2. 切到 API Key
- 点右下角「账号」，选「使用我自己的 API Key」。
- 填入合法 key，点保存并切换。
- 抽屉关闭，状态卡显示「使用我自己的 API Key」。
- 再点首屏大按钮，正常打开。

## 3. 装一个脚本
- 点右上角「更多设置」，展开「增强能力（脚本市场）」。
- 列表加载，点任一脚本「安装」，按钮变「已安装」。

## 4. 触发自动更新（mock 新版）
- 启动 5 秒后，按钮下方出现「发现新版本 vX 立即更新」。
- 点击进入下载，进度模态显示。
- 校验通过 → 安装；校验失败 → 弹「更新校验失败，已拒绝安装」。

## 5. 端口冲突修复
- 在另一个进程占用 57321 后启动应用。
- 按钮停在「错误」态，文案包含端口被占。
- 释放端口后点「再试一次」，回到 ready。

## 6. 卸载快捷方式
- 「更多设置」→「桌面快捷方式 / 卸载」→ 卸载（保留数据）。
- 桌面 / 开始菜单的 Codex Assistant 入口消失，应用本身仍能从 dock 启动。

## 7. 反馈包导出
- 「更多设置」→「反馈包导出」→ 导出。
- 桌面出现 `codex-assistant-diagnostics-*.zip`。

## 8. 重置
- 「更多设置」→「关于 / 重置」→ 重置所有设置。
- 二次确认后，弹「已重置，请重启应用」。
- 重启后回到首次启动状态。
```

- [ ] **Step 2: 提交**

```bash
git add docs/superpowers/specs/2026-05-23-beginner-ux-redesign-manual-smoke.md
git commit -m "docs: add manual smoke test for beginner UX redesign"
```

---

## Task 13: 终验证 + 旧依赖清理

**Files:**
- 仅检查；可能 Modify: `apps/codex-assistant-manager/package.json`

- [ ] **Step 1: 全量前端检查**

```bash
cd apps/codex-assistant-manager && npm run check && npm run test && npm run vite:build
```
Expected: 全部 0 错误，build 成功。

- [ ] **Step 2: 全量 Rust 检查（确认渲染层重写没碰后端）**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy 0 警告；测试全过（与上一个 PR 一致的 256 通过基线）。

- [ ] **Step 3: 检查未使用的前端依赖**

```bash
cd apps/codex-assistant-manager && grep -rE "from \"@dnd-kit|from \"@radix-ui|from \"@tauri-apps/plugin-dialog\"" src/
```
Expected: 输出为空 → 这些依赖在新前端里全部未引用。

- [ ] **Step 4: 移除未使用的前端依赖（基于 Step 3 结果）**

如果 Step 3 输出为空，从 `apps/codex-assistant-manager/package.json` 的 `dependencies` 删除：`@dnd-kit/core`、`@dnd-kit/sortable`、`@dnd-kit/utilities`、`@radix-ui/react-slot`、`@tauri-apps/plugin-dialog`。

> 注：`class-variance-authority`、`clsx`、`tailwind-merge`、`lucide-react` 仍被 `components/ui/*` 间接使用，保留。

Run:
```bash
cd apps/codex-assistant-manager && npm install
```
Expected: lockfile 更新，无错误。

- [ ] **Step 5: 再跑一次完整验证**

```bash
cd apps/codex-assistant-manager && npm run check && npm run test && npm run vite:build
```
Expected: 全部通过。

- [ ] **Step 6: 提交**

```bash
git add apps/codex-assistant-manager/package.json apps/codex-assistant-manager/package-lock.json
git commit -m "chore(manager): drop unused frontend deps after launcher-first rewrite"
```

---

## 完成条件

- [ ] 所有 13 个 task 的 commit 都已落地。
- [ ] `cd apps/codex-assistant-manager && npm run check` 0 错误。
- [ ] `cd apps/codex-assistant-manager && npm run test` ≥ 13 个测试通过。
- [ ] `cd apps/codex-assistant-manager && npm run vite:build` 成功。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0 警告。
- [ ] `cargo test --workspace` 256+ 通过。
- [ ] 手测 `docs/superpowers/specs/2026-05-23-beginner-ux-redesign-manual-smoke.md` 8 项全 PASS。

## 越界禁止

- 不动 `crates/codex-assistant-core/` 任何文件。
- 不动 `apps/codex-assistant-manager/src-tauri/` 任何 Rust 文件。
- 不引入新的前端 UI 库（不加 Radix Dialog、不加 Headless UI、不加 Zustand、不加 SWR）。
- 不写 i18n（中文一套）。

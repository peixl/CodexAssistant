import { useCallback, useEffect, useRef, useReducer } from "react";
import { callSafe } from "@/lib/invoke";
import { TEXT } from "@/lib/text";
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

export type LaunchStatusEnvelope = {
  status?: {
    status?: string;
    message?: string;
    started_at_ms?: number;
  } | null;
  nowMs?: number;
  now_ms?: number;
};

const LAUNCH_POLL_INTERVAL_MS = 400;
const LAUNCH_POLL_TIMEOUT_MS = 75_000;
const LAUNCH_MIN_SPINNER_MS = 250;

export const LAUNCH_POLLING_CONSTANTS = {
  pollIntervalMs: LAUNCH_POLL_INTERVAL_MS,
  pollTimeoutMs: LAUNCH_POLL_TIMEOUT_MS,
  minSpinnerMs: LAUNCH_MIN_SPINNER_MS,
};

type LaunchTerminal =
  | { kind: "running" }
  | { kind: "running_degraded"; message: string }
  | { kind: "failed"; message: string }
  | { kind: "timeout" };

export async function waitForLaunchTerminal(
  launchRequestedAtMs: number,
  options: {
    sleep: (ms: number) => Promise<void>;
    now: () => number;
    pollIntervalMs?: number;
    pollTimeoutMs?: number;
  },
): Promise<LaunchTerminal> {
  const pollIntervalMs = options.pollIntervalMs ?? LAUNCH_POLL_INTERVAL_MS;
  const pollTimeoutMs = options.pollTimeoutMs ?? LAUNCH_POLL_TIMEOUT_MS;
  const deadline = options.now() + pollTimeoutMs;
  while (options.now() < deadline) {
    const r = await callSafe<LaunchStatusEnvelope>("read_launch_status");
    if (r.ok) {
      const status = r.data.status;
      const startedAt =
        typeof status?.started_at_ms === "number" ? status.started_at_ms : 0;
      const fresh = startedAt >= launchRequestedAtMs;
      if (fresh && status?.status === "running") {
        return { kind: "running" };
      }
      if (fresh && status?.status === "running_degraded") {
        return {
          kind: "running_degraded",
          message:
            typeof status.message === "string" && status.message.length > 0
              ? status.message
              : "Codex is running, but some enhancements could not be applied.",
        };
      }
      if (fresh && status?.status === "failed") {
        return {
          kind: "failed",
          message:
            typeof status.message === "string" && status.message.length > 0
              ? status.message
              : TEXT.launcher.launchTimedOut,
        };
      }
    }
    await options.sleep(pollIntervalMs);
  }
  return { kind: "timeout" };
}

function defaultSleep(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

function defaultNow() {
  return Date.now();
}

export function useLauncherMachine(deps: LauncherDeps): {
  state: LauncherState;
  launch: () => Promise<void>;
  retry: () => Promise<void>;
} {
  const [state, dispatch] = useReducer(launcherReducer, initialLauncherState);
  const watcherInstallAttemptedRef = useRef(false);

  useEffect(() => {
    if (!deps.probe) return;
    dispatch({ type: "probe_done", result: deps.probe });
    if (
      deps.probe.hasAccount &&
      !deps.probe.watcherInstalled &&
      !watcherInstallAttemptedRef.current
    ) {
      watcherInstallAttemptedRef.current = true;
      void (async () => {
        await callSafe("install_watcher");
        await callSafe("enable_watcher");
        await deps.onAfterLaunch();
      })();
    }
  }, [deps.probe, deps.onAfterLaunch]);

  const launch = useCallback(async () => {
    dispatch({ type: "launch_click" });
    const spinnerStartedAt = defaultNow();

    let r = await callSafe<Record<string, unknown>>("launch_codex_assistant", {
      request: deps.launchArgs,
    });
    // Only fall back to repair_backend on transport errors (Tauri invoke threw or
    // returned an unknown error code). Backend-reported failures from the
    // launcher (preflight, spawn) have nothing to do with the CLI wrapper.
    if (!r.ok && r.error.code !== "backend_failed") {
      const repair = await callSafe("repair_backend");
      if (repair.ok) {
        r = await callSafe<Record<string, unknown>>("launch_codex_assistant", {
          request: deps.launchArgs,
        });
      }
    }
    if (!r.ok) {
      const elapsed = defaultNow() - spinnerStartedAt;
      if (elapsed < LAUNCH_MIN_SPINNER_MS) {
        await defaultSleep(LAUNCH_MIN_SPINNER_MS - elapsed);
      }
      dispatch({ type: "launch_failed", message: r.error.message });
      return;
    }

    const payload = r.data as { launchRequestedAtMs?: number };
    const launchRequestedAtMs =
      typeof payload?.launchRequestedAtMs === "number"
        ? payload.launchRequestedAtMs
        : defaultNow();

    const terminal = await waitForLaunchTerminal(launchRequestedAtMs, {
      sleep: defaultSleep,
      now: defaultNow,
    });

    const elapsed = defaultNow() - spinnerStartedAt;
    if (elapsed < LAUNCH_MIN_SPINNER_MS) {
      await defaultSleep(LAUNCH_MIN_SPINNER_MS - elapsed);
    }

    if (terminal.kind === "failed" || terminal.kind === "running_degraded") {
      dispatch({ type: "launch_failed", message: terminal.message });
      await deps.onAfterLaunch();
      return;
    }
    if (terminal.kind === "timeout") {
      dispatch({ type: "launch_failed", message: TEXT.launcher.launchTimedOut });
      await deps.onAfterLaunch();
      return;
    }
    dispatch({ type: "launch_done" });
    await deps.onAfterLaunch();
  }, [deps.launchArgs, deps.onAfterLaunch]);

  const retry = useCallback(async () => {
    await launch();
  }, [launch]);

  return { state, launch, retry };
}

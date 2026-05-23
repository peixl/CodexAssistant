import { useCallback, useEffect, useRef, useReducer } from "react";
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
    let r = await callSafe<Record<string, unknown>>("launch_codex_plus", {
      request: deps.launchArgs,
    });
    if (!r.ok) {
      const repair = await callSafe("repair_backend");
      if (repair.ok) {
        r = await callSafe<Record<string, unknown>>("launch_codex_plus", {
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
    await deps.onAfterLaunch();
  }, [deps.onAfterLaunch]);

  return { state, launch, retry };
}

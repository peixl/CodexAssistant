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
    await prepare();
  }, [prepare]);

  return { state, launch, retry };
}

export type ProbeResult = {
  watcherInstalled: boolean;
  watcherEnabled: boolean;
  relayApplied: boolean;
  hasAccount: boolean;
  authenticated: boolean;
  requiresOpenaiAuth: boolean;
};

export type LauncherState =
  | { kind: "preparing" }
  | { kind: "ready" }
  | { kind: "launching" }
  | { kind: "degraded"; message: string }
  | { kind: "need_account" }
  | { kind: "error"; message: string };

export type LauncherEvent =
  | { type: "probe_done"; result: ProbeResult }
  | { type: "prepare_start" }
  | { type: "prepare_done" }
  | { type: "prepare_failed"; message: string }
  | { type: "launch_click" }
  | { type: "launch_done" }
  | { type: "launch_degraded"; message: string }
  | { type: "launch_failed"; message: string }
  | { type: "retry" };

export const initialLauncherState: LauncherState = { kind: "preparing" };

export function deriveStateFromProbe(result: ProbeResult): LauncherState {
  if (!result.hasAccount) return { kind: "need_account" };
  return { kind: "ready" };
}

export function launcherReducer(state: LauncherState, event: LauncherEvent): LauncherState {
  switch (event.type) {
    case "probe_done":
      if (state.kind === "launching") return state;
      if (state.kind === "degraded" && event.result.hasAccount) return state;
      return deriveStateFromProbe(event.result);
    case "prepare_start":
      return { kind: "preparing" };
    case "prepare_done":
      return { kind: "ready" };
    case "prepare_failed":
      return { kind: "error", message: event.message };
    case "launch_click":
      return state.kind === "ready" || state.kind === "error" || state.kind === "degraded"
        ? { kind: "launching" }
        : state;
    case "launch_done":
      return state.kind === "launching" ? { kind: "ready" } : state;
    case "launch_degraded":
      return { kind: "degraded", message: event.message };
    case "launch_failed":
      return { kind: "error", message: event.message };
    case "retry":
      return state.kind === "error" || state.kind === "degraded" ? { kind: "preparing" } : state;
  }
}

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
  authenticated: true,
  requiresOpenaiAuth: false,
};

describe("deriveStateFromProbe", () => {
  it("returns ready when account is set", () => {
    expect(deriveStateFromProbe(fullProbe)).toEqual({ kind: "ready" });
  });
  it("returns need_account when account missing", () => {
    expect(deriveStateFromProbe({ ...fullProbe, hasAccount: false })).toEqual({
      kind: "need_account",
    });
  });
  it("returns ready when account set but watcher uninstalled (watcher is background)", () => {
    expect(
      deriveStateFromProbe({ ...fullProbe, watcherInstalled: false }),
    ).toEqual({ kind: "ready" });
  });
  it("returns ready when account set but relay not applied (relay is optional)", () => {
    expect(
      deriveStateFromProbe({ ...fullProbe, relayApplied: false }),
    ).toEqual({ kind: "ready" });
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

  it("allows launch_click from an error so retry actually starts a new launch", () => {
    expect(
      launcherReducer({ kind: "error", message: "previous" }, { type: "launch_click" }),
    ).toEqual({ kind: "launching" });
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

  it("probe_done derives state from probe", () => {
    expect(
      launcherReducer({ kind: "preparing" }, { type: "probe_done", result: fullProbe }),
    ).toEqual({ kind: "ready" });
  });

  it("probe_done preserves launching state to avoid mid-launch flicker", () => {
    expect(
      launcherReducer({ kind: "launching" }, { type: "probe_done", result: fullProbe }),
    ).toEqual({ kind: "launching" });
  });
});

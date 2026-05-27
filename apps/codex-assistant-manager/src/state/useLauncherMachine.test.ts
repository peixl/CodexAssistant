import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LAUNCH_POLLING_CONSTANTS,
  waitForLaunchTerminal,
  type LaunchStatusEnvelope,
} from "./useLauncherMachine";

type CallSafeResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: { code: string; message: string } };

const invokeMock = vi.hoisted(() =>
  vi.fn<(command: string) => Promise<unknown>>(),
);

vi.mock("@/lib/invoke", async () => {
  return {
    callSafe: async <T>(command: string): Promise<CallSafeResult<T>> => {
      try {
        const data = (await invokeMock(command)) as T;
        return { ok: true, data };
      } catch (error) {
        const message =
          error instanceof Error ? error.message : String(error ?? "unknown");
        return { ok: false, error: { code: "string", message } };
      }
    },
  };
});

function makeClock(startMs: number) {
  let now = startMs;
  return {
    sleep: async (ms: number) => {
      now += ms;
    },
    now: () => now,
    advance: (ms: number) => {
      now += ms;
    },
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("waitForLaunchTerminal", () => {
  it("returns running when fresh status flips to running", async () => {
    const launchAt = 1_000_000;
    invokeMock
      .mockResolvedValueOnce({
        status: { status: "running", started_at_ms: launchAt + 50 },
        now_ms: launchAt + 100,
      } satisfies LaunchStatusEnvelope)
      .mockRejectedValue(new Error("should not be polled again"));

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
    });

    expect(result).toEqual({ kind: "running" });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("returns failed with backend message when fresh status reports failed", async () => {
    const launchAt = 2_000_000;
    invokeMock.mockResolvedValueOnce({
      status: {
        status: "failed",
        message: "未找到 Codex App",
        started_at_ms: launchAt + 5,
      },
      now_ms: launchAt + 10,
    } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
    });

    expect(result).toEqual({ kind: "failed", message: "未找到 Codex App" });
  });

  it("returns running_degraded instead of failure when Codex launched in compatibility mode", async () => {
    const launchAt = 2_500_000;
    invokeMock.mockResolvedValueOnce({
      status: {
        status: "running_degraded",
        message: "Codex launched in compatibility mode because Windows TCP loopback is blocked.",
        started_at_ms: launchAt + 5,
      },
      now_ms: launchAt + 10,
    } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
    });

    expect(result).toEqual({
      kind: "running_degraded",
      message: "Codex launched in compatibility mode because Windows TCP loopback is blocked.",
    });
  });

  it("falls back to timeout text when failed status carries no message", async () => {
    const launchAt = 3_000_000;
    invokeMock.mockResolvedValueOnce({
      status: { status: "failed", started_at_ms: launchAt + 1 },
    } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
    });

    expect(result.kind).toBe("failed");
    if (result.kind === "failed") {
      expect(result.message.length).toBeGreaterThan(0);
    }
  });

  it("ignores stale statuses written before this launch attempt", async () => {
    const launchAt = 4_000_000;
    invokeMock
      .mockResolvedValueOnce({
        status: { status: "failed", message: "previous", started_at_ms: launchAt - 5_000 },
      } satisfies LaunchStatusEnvelope)
      .mockResolvedValueOnce({
        status: { status: "running", started_at_ms: launchAt + 100 },
      } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
    });

    expect(result).toEqual({ kind: "running" });
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("returns timeout when no terminal status appears before the deadline", async () => {
    const launchAt = 5_000_000;
    invokeMock.mockResolvedValue({
      status: { status: "pending", started_at_ms: launchAt + 1 },
    } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
      pollIntervalMs: 400,
      pollTimeoutMs: 1_200,
    });

    expect(result).toEqual({ kind: "timeout" });
  });

  it("continues polling when callSafe reports a transient failure", async () => {
    const launchAt = 6_000_000;
    invokeMock
      .mockRejectedValueOnce(new Error("ipc bounced"))
      .mockResolvedValueOnce({
        status: { status: "running", started_at_ms: launchAt + 200 },
      } satisfies LaunchStatusEnvelope);

    const clock = makeClock(launchAt);
    const result = await waitForLaunchTerminal(launchAt, {
      sleep: clock.sleep,
      now: clock.now,
      pollIntervalMs: 100,
      pollTimeoutMs: 5_000,
    });

    expect(result).toEqual({ kind: "running" });
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});

describe("LAUNCH_POLLING_CONSTANTS", () => {
  it("exposes the polling parameters used by the hook", () => {
    expect(LAUNCH_POLLING_CONSTANTS.pollIntervalMs).toBeGreaterThan(0);
    expect(LAUNCH_POLLING_CONSTANTS.pollTimeoutMs).toBeGreaterThan(
      LAUNCH_POLLING_CONSTANTS.pollIntervalMs,
    );
    expect(LAUNCH_POLLING_CONSTANTS.pollTimeoutMs).toBeGreaterThanOrEqual(60_000);
    expect(LAUNCH_POLLING_CONSTANTS.minSpinnerMs).toBeGreaterThan(0);
  });
});

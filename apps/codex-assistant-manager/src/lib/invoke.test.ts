import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  invokeMock.mockReset();
});

import {
  callSafe,
  call,
  extractBackendError,
  isBackendFailure,
  normalizeInvokeError,
} from "./invoke";

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

describe("isBackendFailure", () => {
  it("detects status=failed envelope", () => {
    expect(isBackendFailure({ status: "failed", message: "x" })).toBe(true);
  });

  it("does not match status=ok / accepted / not_checked", () => {
    expect(isBackendFailure({ status: "ok", message: "x" })).toBe(false);
    expect(isBackendFailure({ status: "accepted" })).toBe(false);
    expect(isBackendFailure({ status: "not_checked" })).toBe(false);
  });

  it("does not match non-objects", () => {
    expect(isBackendFailure(null)).toBe(false);
    expect(isBackendFailure(undefined)).toBe(false);
    expect(isBackendFailure("failed")).toBe(false);
    expect(isBackendFailure(42)).toBe(false);
  });
});

describe("extractBackendError", () => {
  it("returns backend message verbatim", () => {
    expect(extractBackendError({ status: "failed", message: "boom on backend" })).toEqual({
      code: "backend_failed",
      message: "boom on backend",
    });
  });

  it("falls back to TEXT.errors.unknown when message missing or empty", () => {
    expect(extractBackendError({ status: "failed" }).message).toBe("出错了，请稍后再试");
    expect(extractBackendError({ status: "failed", message: "" }).message).toBe("出错了，请稍后再试");
  });
});

describe("callSafe", () => {
  it("returns ok=true when backend resolves with status=ok envelope", async () => {
    invokeMock.mockResolvedValueOnce({ status: "ok", message: "done", payload: 1 });
    const r = await callSafe<{ status: string; payload: number }>("noop");
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.data.payload).toBe(1);
  });

  it("returns ok=false when backend resolves with status=failed envelope", async () => {
    invokeMock.mockResolvedValueOnce({
      status: "failed",
      message: "启动静默入口失败：no such file",
    });
    const r = await callSafe("launch_codex_assistant");
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.error.code).toBe("backend_failed");
      expect(r.error.message).toContain("no such file");
    }
  });

  it("returns ok=false when invoke throws", async () => {
    invokeMock.mockRejectedValueOnce(new Error("kaboom"));
    const r = await callSafe("noop");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.message).toContain("kaboom");
  });

  it("treats status=accepted as success", async () => {
    invokeMock.mockResolvedValueOnce({ status: "accepted", message: "queued" });
    const r = await callSafe("launch_codex_assistant");
    expect(r.ok).toBe(true);
  });
});

describe("call", () => {
  it("returns data on success", async () => {
    invokeMock.mockResolvedValueOnce({ status: "ok", payload: { x: 7 } });
    const r = await call<{ payload: { x: number } }>("noop");
    expect(r.payload.x).toBe(7);
  });

  it("throws when backend reports failure envelope", async () => {
    invokeMock.mockResolvedValueOnce({ status: "failed", message: "broken" });
    await expect(call("noop")).rejects.toThrow("broken");
  });
});

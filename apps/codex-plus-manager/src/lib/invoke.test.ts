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

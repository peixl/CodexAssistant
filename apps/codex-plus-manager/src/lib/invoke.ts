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

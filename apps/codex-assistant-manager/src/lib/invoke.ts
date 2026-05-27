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

type BackendEnvelope = { status?: unknown; message?: unknown };
const TAURI_UNAVAILABLE_CODE = "tauri_unavailable";

function tauriUnavailableError(): NormalizedError {
  return { code: TAURI_UNAVAILABLE_CODE, message: TEXT.errors.tauriUnavailable };
}

function isTauriRuntimeAvailable(): boolean {
  if (typeof window === "undefined") return true;
  return "__TAURI_INTERNALS__" in window;
}

export function isBackendFailure(value: unknown): value is BackendEnvelope {
  if (!value || typeof value !== "object") return false;
  const status = (value as BackendEnvelope).status;
  return typeof status === "string" && status === "failed";
}

export function extractBackendError(value: unknown): NormalizedError {
  const message =
    typeof (value as BackendEnvelope)?.message === "string" &&
    ((value as BackendEnvelope).message as string).length > 0
      ? ((value as BackendEnvelope).message as string)
      : TEXT.errors.unknown;
  return { code: "backend_failed", message };
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntimeAvailable()) {
    const error = tauriUnavailableError();
    throw Object.assign(new Error(error.message), { code: error.code });
  }
  const data = await tauriInvoke<T>(command, args);
  if (isBackendFailure(data)) {
    const error = extractBackendError(data);
    throw Object.assign(new Error(error.message), { code: error.code });
  }
  return data;
}

export async function callSafe<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<{ ok: true; data: T } | { ok: false; error: NormalizedError }> {
  if (!isTauriRuntimeAvailable()) {
    return { ok: false, error: tauriUnavailableError() };
  }
  try {
    const data = await tauriInvoke<T>(command, args);
    if (isBackendFailure(data)) {
      return { ok: false, error: extractBackendError(data) };
    }
    return { ok: true, data };
  } catch (error) {
    return { ok: false, error: normalizeInvokeError(error) };
  }
}

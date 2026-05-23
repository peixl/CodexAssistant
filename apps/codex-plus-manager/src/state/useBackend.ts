import { useCallback, useEffect, useState } from "react";
import { callSafe, type NormalizedError } from "@/lib/invoke";
import type { ProbeResult } from "./launcherMachine";

export type OverviewLite = {
  hasAccount: boolean;
  appPath: string | null;
  debugPort: number;
  helperPort: number;
};

export type SettingsLite = {
  hasApiKey: boolean;
  apiKey: string;
  baseUrl: string;
  activeRelayId: string;
  raw: Record<string, unknown>;
};

type RelayProfileLite = {
  id?: string;
  apiKey?: string;
  baseUrl?: string;
  [key: string]: unknown;
};

type SettingsBlob = {
  activeRelayId?: string;
  relayProfiles?: RelayProfileLite[];
  [key: string]: unknown;
};

function extractActiveProfile(raw: SettingsBlob): {
  apiKey: string;
  baseUrl: string;
  activeRelayId: string;
} {
  const activeRelayId = typeof raw.activeRelayId === "string" ? raw.activeRelayId : "";
  const profiles = Array.isArray(raw.relayProfiles) ? raw.relayProfiles : [];
  const active = profiles.find((p) => p?.id === activeRelayId) ?? profiles[0];
  return {
    apiKey: (active?.apiKey ?? "").toString().trim(),
    baseUrl: (active?.baseUrl ?? "").toString().trim(),
    activeRelayId,
  };
}

export function mergeApiKeyIntoSettings(
  raw: SettingsBlob,
  apiKey: string,
  baseUrl: string,
): SettingsBlob {
  const profiles: RelayProfileLite[] = Array.isArray(raw.relayProfiles) ? [...raw.relayProfiles] : [];
  const activeId = typeof raw.activeRelayId === "string" ? raw.activeRelayId : "";
  let idx = profiles.findIndex((p) => p?.id === activeId);
  if (idx < 0) idx = profiles.length > 0 ? 0 : -1;
  if (idx < 0) {
    profiles.push({ id: activeId || "default", apiKey, baseUrl });
  } else {
    profiles[idx] = {
      ...profiles[idx],
      apiKey,
      ...(baseUrl ? { baseUrl } : {}),
    };
  }
  return { ...raw, relayProfiles: profiles };
}

export const extractActiveProfileForTest = extractActiveProfile;
export const mergeApiKeyForTest = mergeApiKeyIntoSettings;

async function loadProbe(): Promise<
  { probe: ProbeResult; overview: OverviewLite; settings: SettingsLite; error: NormalizedError | null }
> {
  const overview = await callSafe<Record<string, unknown>>("load_overview");
  const watcher = await callSafe<Record<string, unknown>>("load_watcher_state");
  const relay = await callSafe<Record<string, unknown>>("relay_status");
  const settings = await callSafe<Record<string, unknown>>("load_settings");

  const firstError =
    !overview.ok ? overview.error
    : !watcher.ok ? watcher.error
    : !relay.ok ? relay.error
    : !settings.ok ? settings.error
    : null;

  const ov = (overview.ok ? overview.data : {}) as {
    codex_app?: { status?: string; path?: string | null };
  };
  const wa = (watcher.ok ? watcher.data : {}) as { installed?: boolean; enabled?: boolean };
  const re = (relay.ok ? relay.data : {}) as {
    authenticated?: boolean;
    configured?: boolean;
    requiresOpenaiAuth?: boolean;
  };
  const stRaw = (settings.ok ? settings.data : {}) as { settings?: Record<string, unknown> };
  const rawSettings = (stRaw.settings ?? {}) as Record<string, unknown>;
  const { apiKey, baseUrl, activeRelayId } = extractActiveProfile(rawSettings);

  const hasApiKey = apiKey.length > 0;
  const hasAccount = !!re.authenticated || hasApiKey;

  return {
    overview: {
      hasAccount,
      appPath: ov.codex_app?.path ?? null,
      debugPort: 9229,
      helperPort: 57321,
    },
    probe: {
      watcherInstalled: !!wa.installed,
      watcherEnabled: !!wa.enabled,
      relayApplied: !!re.configured,
      hasAccount,
      authenticated: !!re.authenticated,
      requiresOpenaiAuth: !!re.requiresOpenaiAuth,
    },
    settings: { hasApiKey, apiKey, baseUrl, activeRelayId, raw: rawSettings },
    error: firstError,
  };
}

export function useBackend() {
  const [overview, setOverview] = useState<OverviewLite | null>(null);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [settings, setSettings] = useState<SettingsLite | null>(null);
  const [error, setError] = useState<NormalizedError | null>(null);

  const refresh = useCallback(async () => {
    const result = await loadProbe();
    setOverview(result.overview);
    setProbe(result.probe);
    setSettings(result.settings);
    setError(result.error);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { overview, probe, settings, error, refresh };
}

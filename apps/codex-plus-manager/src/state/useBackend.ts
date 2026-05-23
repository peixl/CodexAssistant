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
};

async function loadProbe(): Promise<
  | { probe: ProbeResult; overview: OverviewLite; settings: SettingsLite }
  | NormalizedError
> {
  const overview = await callSafe<Record<string, unknown>>("load_overview");
  if (!overview.ok) return overview.error;
  const watcher = await callSafe<Record<string, unknown>>("load_watcher_state");
  if (!watcher.ok) return watcher.error;
  const relay = await callSafe<Record<string, unknown>>("relay_status");
  if (!relay.ok) return relay.error;
  const settings = await callSafe<Record<string, unknown>>("load_settings");
  if (!settings.ok) return settings.error;

  const ov = overview.data as {
    codex_app?: { status?: string; path?: string | null };
  };
  const wa = watcher.data as { installed?: boolean; enabled?: boolean };
  const re = relay.data as { authenticated?: boolean; configured?: boolean };
  const stRaw = settings.data as { settings?: Record<string, unknown> };
  const st = (stRaw.settings ?? {}) as { officialMixApiKey?: string|null; officialMixBaseUrl?: string|null };

  const apiKey = (st.officialMixApiKey ?? "").trim();
  const baseUrl = (st.officialMixBaseUrl ?? "").trim();
  const hasAccount = !!re.authenticated || apiKey.length > 0;

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
    },
    settings: { hasApiKey: apiKey.length > 0, apiKey, baseUrl },
  };
}

export function useBackend() {
  const [overview, setOverview] = useState<OverviewLite | null>(null);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [settings, setSettings] = useState<SettingsLite | null>(null);
  const [error, setError] = useState<NormalizedError | null>(null);

  const refresh = useCallback(async () => {
    const result = await loadProbe();
    if ("probe" in result) {
      setOverview(result.overview);
      setProbe(result.probe);
      setSettings(result.settings);
      setError(null);
    } else {
      setError(result);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { overview, probe, settings, error, refresh };
}

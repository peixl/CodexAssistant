import { useEffect, useState } from "react";
import { Drawer } from "@/components/Drawer";
import { TEXT } from "@/lib/text";
import { callSafe } from "@/lib/invoke";
import type { RelayKind } from "@/state/useLauncherMachine";
import { mergeApiKeyIntoSettings } from "@/state/useBackend";

type RelayProfile = {
  id?: string;
  apiKey?: string;
  baseUrl?: string;
  [key: string]: unknown;
};

type SettingsBlob = {
  activeRelayId?: string;
  relayProfiles?: RelayProfile[];
  [key: string]: unknown;
};

async function loadRawSettings(): Promise<SettingsBlob> {
  const r = await callSafe<{ settings?: SettingsBlob }>("load_settings");
  if (!r.ok) return {};
  return r.data.settings ?? {};
}

export function AccountDrawer({
  open,
  onClose,
  current,
  onApplied,
}: {
  open: boolean;
  onClose: () => void;
  current: RelayKind;
  onApplied: () => Promise<void>;
}) {
  const [kind, setKind] = useState<RelayKind>(current);
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setKind(current === "none" ? "chatgpt" : current);
    setError(null);
    void (async () => {
      const raw = await loadRawSettings();
      const profiles = Array.isArray(raw.relayProfiles) ? raw.relayProfiles : [];
      const activeId = typeof raw.activeRelayId === "string" ? raw.activeRelayId : "";
      const active = profiles.find((p) => p?.id === activeId) ?? profiles[0];
      setApiKey((active?.apiKey ?? "").toString());
      setBaseUrl((active?.baseUrl ?? "").toString());
    })();
  }, [open, current]);

  const openLogin = async () => {
    setBusy(true); setError(null);
    const r = await callSafe("apply_relay_injection");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    await onApplied();
    onClose();
  };

  const saveApiKey = async () => {
    setBusy(true); setError(null);
    const trimmed = apiKey.trim();
    if (!trimmed) { setBusy(false); setError("API Key 不能为空"); return; }
    const raw = await loadRawSettings();
    const merged = mergeApiKeyIntoSettings(raw, trimmed, baseUrl.trim());
    const save = await callSafe("save_settings", { settings: merged });
    if (!save.ok) { setBusy(false); setError(save.error.message); return; }
    const apply = await callSafe("apply_pure_api_injection");
    setBusy(false);
    if (!apply.ok) { setError(apply.error.message); return; }
    await onApplied();
    onClose();
  };

  return (
    <Drawer open={open} title={TEXT.account.title} onClose={onClose}>
      <div className="space-y-6">
        <label className="flex items-start gap-3">
          <input type="radio" checked={kind === "chatgpt"} onChange={() => setKind("chatgpt")} className="mt-1" />
          <div className="flex-1">
            <div>{TEXT.account.chatgpt}</div>
            {kind === "chatgpt" && (
              <button onClick={openLogin} disabled={busy} className="mt-2 px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm disabled:opacity-60">
                {busy ? "处理中…" : TEXT.account.openLogin}
              </button>
            )}
          </div>
        </label>

        <label className="flex items-start gap-3">
          <input type="radio" checked={kind === "apiKey"} onChange={() => setKind("apiKey")} className="mt-1" />
          <div className="flex-1 space-y-2">
            <div>{TEXT.account.apiKey}</div>
            {kind === "apiKey" && (
              <>
                <input
                  value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..."
                  className="w-full px-2 py-1 border border-border rounded bg-background"
                />
                <input
                  value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="Base URL（可选）"
                  className="w-full px-2 py-1 border border-border rounded bg-background"
                />
                <button onClick={saveApiKey} disabled={busy} className="px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm disabled:opacity-60">
                  {busy ? "处理中…" : TEXT.account.saveSwitch}
                </button>
              </>
            )}
          </div>
        </label>

        {error && <p className="text-sm text-destructive">{error}</p>}
        <p className="text-xs text-muted-foreground">
          {TEXT.account.current}：{current === "apiKey" ? TEXT.account.apiKey : current === "chatgpt" ? TEXT.account.chatgpt : "未配置"}
        </p>
      </div>
    </Drawer>
  );
}

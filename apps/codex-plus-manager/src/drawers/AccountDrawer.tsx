import { useEffect, useState } from "react";
import { KeyRound, Link, RefreshCw, Save } from "lucide-react";
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

type CodexCredentialsPayload = {
  apiKey?: string;
  baseUrl?: string;
  apiKeySource?: string;
  baseUrlSource?: string;
  codexHome?: string;
};

const DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";

async function loadRawSettings(): Promise<SettingsBlob> {
  const r = await callSafe<{ settings?: SettingsBlob }>("load_settings");
  if (!r.ok) return {};
  return r.data.settings ?? {};
}

async function loadCodexCredentials(): Promise<CodexCredentialsPayload> {
  const r = await callSafe<CodexCredentialsPayload>("read_codex_credentials");
  if (!r.ok) return {};
  return r.data;
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
  const [credentialNote, setCredentialNote] = useState<string | null>(null);
  const [credentialLoading, setCredentialLoading] = useState(false);
  const [codexHome, setCodexHome] = useState("");

  const refreshCredentials = async () => {
    setCredentialLoading(true);
    setError(null);
    setCredentialNote(null);
    const [raw, codex] = await Promise.all([
      loadRawSettings(),
      loadCodexCredentials(),
    ]);
    const profiles = Array.isArray(raw.relayProfiles) ? raw.relayProfiles : [];
    const activeId = typeof raw.activeRelayId === "string" ? raw.activeRelayId : "";
    const active = profiles.find((p) => p?.id === activeId) ?? profiles[0];
    const profileKey = (active?.apiKey ?? "").toString().trim();
    const profileBase = (active?.baseUrl ?? "").toString().trim();
    const codexKey = (codex.apiKey ?? "").toString().trim();
    const codexBase = (codex.baseUrl ?? "").toString().trim();

    const resolvedKey = profileKey || codexKey;
    const resolvedBase = profileBase || codexBase || (resolvedKey ? DEFAULT_OPENAI_BASE_URL : "");
    setApiKey(resolvedKey);
    setBaseUrl(resolvedBase);
    setCodexHome((codex.codexHome ?? "").toString());

    const usedCodexKey = !profileKey && codexKey.length > 0;
    const usedCodexBase = !profileBase && codexBase.length > 0;
    const usedDefaultBase = !profileBase && !codexBase && resolvedKey.length > 0;
    if (usedCodexKey || usedCodexBase || usedDefaultBase) {
      const parts: string[] = [];
      if (usedCodexKey) parts.push("API Key");
      if (usedCodexBase) parts.push("Base URL");
      if (usedDefaultBase) parts.push("官方默认 Base URL");
      setCredentialNote(`已自动填入${parts.join(" 与 ")}，确认后可直接保存切换。`);
    } else if (resolvedKey || resolvedBase) {
      setCredentialNote("已载入当前保存的 API 配置，可直接保存或调整。");
    } else {
      setCredentialNote("未读取到本地 API 配置；可先在 Codex 中配置，或在此处填写一次。");
    }
    if (current === "none" && resolvedKey.length > 0) {
      setKind("apiKey");
    }
    setCredentialLoading(false);
  };

  useEffect(() => {
    if (!open) return;
    setKind(current === "none" ? "chatgpt" : current);
    setError(null);
    setCredentialNote(null);
    void refreshCredentials();
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
    const trimmedBaseUrl = baseUrl.trim() || DEFAULT_OPENAI_BASE_URL;
    const raw = await loadRawSettings();
    const merged = mergeApiKeyIntoSettings(raw, trimmed, trimmedBaseUrl);
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
            <div className="flex items-center justify-between gap-3">
              <span>{TEXT.account.apiKey}</span>
              <button
                type="button"
                onClick={refreshCredentials}
                disabled={busy || credentialLoading}
                className="inline-flex items-center gap-1 text-xs text-primary hover:underline disabled:opacity-60 disabled:no-underline"
              >
                <RefreshCw className={`size-3.5 ${credentialLoading ? "animate-spin" : ""}`} />
                {TEXT.account.readCodex}
              </button>
            </div>
            {kind === "apiKey" && (
              <>
                <label className="field compact-field">
                  <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                    <KeyRound className="size-3.5" /> API Key
                  </span>
                  <input
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-..."
                    autoComplete="off"
                    className="w-full px-2 py-1 border border-border rounded bg-background"
                  />
                </label>
                <label className="field compact-field">
                  <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
                    <Link className="size-3.5" /> Base URL
                  </span>
                  <input
                    value={baseUrl}
                    onChange={(e) => setBaseUrl(e.target.value)}
                    placeholder={DEFAULT_OPENAI_BASE_URL}
                    autoComplete="off"
                    className="w-full px-2 py-1 border border-border rounded bg-background"
                  />
                </label>
                <button
                  onClick={saveApiKey}
                  disabled={busy || credentialLoading || !apiKey.trim()}
                  className="inline-flex items-center gap-1 px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm disabled:opacity-60"
                >
                  <Save className="size-4" />
                  {busy ? "处理中…" : TEXT.account.saveSwitch}
                </button>
                {credentialNote && (
                  <p className="text-xs text-muted-foreground">{credentialNote}</p>
                )}
                {codexHome && (
                  <p className="text-xs text-muted-foreground break-all">
                    {TEXT.account.codexHome}：{codexHome}
                  </p>
                )}
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

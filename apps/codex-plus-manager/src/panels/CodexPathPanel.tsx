import { useCallback, useEffect, useState } from "react";
import { Folder, RefreshCw, Save } from "lucide-react";
import { TEXT } from "@/lib/text";
import { callSafe } from "@/lib/invoke";

type SettingsBlob = { codexAppPath?: string; [key: string]: unknown };

type CodexAppPathPayload = {
  path: string | null;
  version: string | null;
};

async function loadSettingsBlob(): Promise<SettingsBlob> {
  const r = await callSafe<{ settings?: SettingsBlob }>("load_settings");
  if (!r.ok) return {};
  return r.data.settings ?? {};
}

export function CodexPathPanel() {
  const [savedPath, setSavedPath] = useState("");
  const [inputPath, setInputPath] = useState("");
  const [detectedPath, setDetectedPath] = useState<string | null>(null);
  const [detectedVersion, setDetectedVersion] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ kind: "info" | "error"; text: string } | null>(null);
  const [rawSettings, setRawSettings] = useState<SettingsBlob>({});

  const refresh = useCallback(async () => {
    setBusy(true);
    const settings = await loadSettingsBlob();
    setRawSettings(settings);
    const saved = (settings.codexAppPath ?? "").toString().trim();
    setSavedPath(saved);
    setInputPath(saved);
    const detected = await callSafe<CodexAppPathPayload>("detect_codex_app_path");
    if (detected.ok) {
      setDetectedPath(detected.data.path);
      setDetectedVersion(detected.data.version);
      if (!saved && detected.data.path) {
        setInputPath(detected.data.path);
      }
    } else {
      setDetectedPath(null);
      setDetectedVersion(null);
    }
    setBusy(false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const browse = async () => {
    setBusy(true);
    setMessage(null);
    const picked = await callSafe<CodexAppPathPayload>("pick_codex_app_path");
    setBusy(false);
    if (!picked.ok) {
      setMessage({ kind: "error", text: picked.error.message });
      return;
    }
    if (picked.data.path) {
      setInputPath(picked.data.path);
      setMessage({ kind: "info", text: TEXT.codexPath.picked });
    }
  };

  const save = async () => {
    setBusy(true);
    setMessage(null);
    const next: SettingsBlob = { ...rawSettings, codexAppPath: inputPath.trim() };
    const r = await callSafe<{ settings?: SettingsBlob }>("save_settings", { settings: next });
    setBusy(false);
    if (!r.ok) {
      setMessage({ kind: "error", text: r.error.message });
      return;
    }
    const saved = (r.data.settings?.codexAppPath ?? "").toString();
    setSavedPath(saved);
    setInputPath(saved);
    setRawSettings(r.data.settings ?? next);
    setMessage({ kind: "info", text: TEXT.codexPath.saved });
  };

  const useDetected = () => {
    if (detectedPath) setInputPath(detectedPath);
  };

  const dirty = inputPath.trim() !== savedPath.trim();

  return (
    <div className="space-y-3">
      <div className="space-y-1">
        <h3 className="text-sm font-medium">{TEXT.codexPath.title}</h3>
        <p className="text-xs text-muted-foreground">{TEXT.codexPath.hint}</p>
      </div>

      <div className="space-y-2">
        <label className="block text-xs text-muted-foreground">{TEXT.codexPath.inputLabel}</label>
        <div className="flex gap-2">
          <input
            type="text"
            value={inputPath}
            onChange={(e) => setInputPath(e.target.value)}
            placeholder={TEXT.codexPath.placeholder}
            disabled={busy}
            className="flex-1 px-2 py-1 border border-border rounded bg-background font-mono text-xs"
          />
          <button
            type="button"
            onClick={browse}
            disabled={busy}
            className="inline-flex items-center gap-1 px-3 py-1.5 rounded border border-border text-sm hover:bg-muted disabled:opacity-60"
          >
            <Folder className="size-4" /> {TEXT.codexPath.browse}
          </button>
        </div>
      </div>

      {detectedPath && (
        <div className="rounded border border-border bg-muted/40 px-3 py-2 text-xs space-y-1">
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">{TEXT.codexPath.detected}</span>
            <button
              type="button"
              onClick={useDetected}
              disabled={busy || inputPath.trim() === detectedPath}
              className="text-primary hover:underline disabled:opacity-50 disabled:no-underline"
            >
              {TEXT.codexPath.useDetected}
            </button>
          </div>
          <div className="font-mono break-all">{detectedPath}</div>
          {detectedVersion && (
            <div className="text-muted-foreground">{TEXT.codexPath.version} {detectedVersion}</div>
          )}
        </div>
      )}

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={save}
          disabled={busy || !dirty}
          className="inline-flex items-center gap-1 px-3 py-1.5 rounded bg-primary text-primary-foreground text-sm disabled:opacity-60"
        >
          <Save className="size-4" /> {TEXT.codexPath.save}
        </button>
        <button
          type="button"
          onClick={refresh}
          disabled={busy}
          className="inline-flex items-center gap-1 px-3 py-1.5 rounded border border-border text-sm hover:bg-muted disabled:opacity-60"
        >
          <RefreshCw className={`size-4 ${busy ? "animate-spin" : ""}`} /> {TEXT.codexPath.refresh}
        </button>
      </div>

      {message && (
        <p className={`text-xs ${message.kind === "error" ? "text-destructive" : "text-muted-foreground"}`}>
          {message.text}
        </p>
      )}
    </div>
  );
}

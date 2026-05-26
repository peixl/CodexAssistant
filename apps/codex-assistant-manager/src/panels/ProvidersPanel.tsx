import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function ProvidersPanel() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const importNow = async () => {
    setBusy(true); setError(null); setMsg(null);
    const r = await callSafe<{ message?: string }>("import_ccs_providers");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setMsg(r.data.message ?? "已导入");
  };

  const sync = async () => {
    setBusy(true); setError(null); setMsg(null);
    const r = await callSafe<{ message?: string }>("sync_providers_now");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setMsg("同步完成");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">服务源（CCS）</h3>
      <div className="flex gap-2">
        <button onClick={importNow} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">导入 CCS 配置</button>
        <button onClick={sync} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">立即同步</button>
      </div>
      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

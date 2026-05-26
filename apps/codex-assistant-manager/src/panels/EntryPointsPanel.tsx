import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function EntryPointsPanel() {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async (cmd: string, args?: Record<string, unknown>) => {
    setBusy(true); setError(null); setMsg(null);
    const r = await callSafe<{ message?: string }>(cmd, args);
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setMsg(r.data.message ?? "完成");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">桌面快捷方式</h3>
      <div className="flex flex-wrap gap-2">
        <button onClick={() => run("install_entrypoints")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">安装</button>
        <button onClick={() => run("repair_shortcuts")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">修复</button>
        <button onClick={() => run("uninstall_entrypoints", { options: { keepData: true } })} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">卸载（保留数据）</button>
      </div>
      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

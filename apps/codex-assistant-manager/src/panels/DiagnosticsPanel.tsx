import { useState } from "react";
import { callSafe } from "@/lib/invoke";

export function DiagnosticsPanel() {
  const [busy, setBusy] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const exportPack = async () => {
    setBusy(true); setError(null); setPath(null);
    const r = await callSafe<{ path?: string }>("copy_diagnostics");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setPath(r.data.path ?? null);
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">反馈包</h3>
      <button onClick={exportPack} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">
        导出反馈包到桌面
      </button>
      {path && <p className="text-xs text-muted-foreground">已导出：{path}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

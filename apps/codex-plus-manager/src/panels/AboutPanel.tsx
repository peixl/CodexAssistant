import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

export function AboutPanel() {
  const [version, setVersion] = useState<string>("...");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const r = await callSafe<{ version: string }>("backend_version");
      if (r.ok) setVersion(r.data.version);
    })();
  }, []);

  const reset = async () => {
    if (!confirm("确认重置所有设置？此操作不可撤销。")) return;
    setBusy(true); setError(null);
    const r = await callSafe("reset_settings");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    alert("已重置，请重启应用");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">关于</h3>
      <p className="text-xs text-muted-foreground">版本：{version}</p>
      <button onClick={reset} disabled={busy} className="text-xs px-2 py-1 rounded border border-destructive text-destructive">
        重置所有设置
      </button>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

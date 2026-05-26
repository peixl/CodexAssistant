import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

type MarketItem = { id: string; name: string; description?: string; version: string; author?: string };
type Payload = { market?: { scripts: MarketItem[] }; installed?: Record<string, { enabled: boolean }> };

export function ScriptsPanel() {
  const [data, setData] = useState<Payload | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setBusy(true); setError(null);
    const r = await callSafe<Payload>("refresh_script_market");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setData(r.data);
  };

  useEffect(() => { void refresh(); }, []);

  const install = async (id: string) => {
    setBusy(true); setError(null);
    const r = await callSafe<Payload>("install_market_script", { id });
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setData(r.data);
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">脚本市场</h3>
        <button onClick={refresh} disabled={busy} className="text-xs underline text-primary">刷新</button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
      <ul className="space-y-2">
        {(data?.market?.scripts ?? []).map((item) => {
          const installed = !!data?.installed?.[item.id];
          return (
            <li key={item.id} className="flex items-center justify-between border border-border rounded px-3 py-2">
              <div>
                <div className="text-sm font-medium">{item.name} <span className="text-xs text-muted-foreground">v{item.version}</span></div>
                {item.description && <div className="text-xs text-muted-foreground">{item.description}</div>}
              </div>
              <button onClick={() => install(item.id)} disabled={busy || installed} className="text-xs px-2 py-1 rounded bg-primary text-primary-foreground disabled:opacity-50">
                {installed ? "已安装" : "安装"}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

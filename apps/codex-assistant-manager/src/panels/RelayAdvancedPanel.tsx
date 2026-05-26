import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

type Files = { configContents: string; authContents: string };

export function RelayAdvancedPanel() {
  const [files, setFiles] = useState<Files>({ configContents: "", authContents: "" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const r = await callSafe<Files>("read_relay_files");
      if (r.ok) setFiles(r.data);
    })();
  }, []);

  const save = async (target: "config" | "auth") => {
    setBusy(true); setError(null); setMsg(null);
    const contents = target === "config" ? files.configContents : files.authContents;
    const r = await callSafe<Files>("save_relay_file", { request: { target, contents } });
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setFiles(r.data); setMsg("已保存");
  };

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium">中转配置文件</h3>
      <p className="text-xs text-muted-foreground">高级选项；除非你知道在改什么，否则不要动。</p>

      <label className="text-xs">config.toml</label>
      <textarea
        value={files.configContents}
        onChange={(e) => setFiles({ ...files, configContents: e.target.value })}
        className="w-full h-32 px-2 py-1 border border-border rounded bg-background font-mono text-xs"
      />
      <button onClick={() => save("config")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">保存 config</button>

      <label className="text-xs">auth.json</label>
      <textarea
        value={files.authContents}
        onChange={(e) => setFiles({ ...files, authContents: e.target.value })}
        className="w-full h-32 px-2 py-1 border border-border rounded bg-background font-mono text-xs"
      />
      <button onClick={() => save("auth")} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">保存 auth</button>

      {msg && <p className="text-xs text-muted-foreground">{msg}</p>}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

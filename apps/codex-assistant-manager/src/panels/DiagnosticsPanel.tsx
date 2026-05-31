import { useState } from "react";
import { callSafe } from "@/lib/invoke";
import { AlertCircle, CheckCircle, Loader2 } from "lucide-react";

export function DiagnosticsPanel() {
  const [busy, setBusy] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loopbackTest, setLoopbackTest] = useState<{
    status: "success" | "failed" | null;
    message: string;
  }>({ status: null, message: "" });

  const exportPack = async () => {
    setBusy(true); setError(null); setPath(null);
    const r = await callSafe<{ path?: string }>("copy_diagnostics");
    setBusy(false);
    if (!r.ok) { setError(r.error.message); return; }
    setPath(r.data.path ?? null);
  };

  const testLoopback = async () => {
    setBusy(true);
    setLoopbackTest({ status: null, message: "正在测试本地回环连接..." });

    const r = await callSafe<{
      status: string;
      message: string;
      diagnostic?: string;
    }>("test_loopback_connectivity");

    setBusy(false);

    if (r.ok && r.data.status === "ok") {
      setLoopbackTest({
        status: "success",
        message: r.data.message || "✓ 本地回环连接正常"
      });
    } else {
      const diagnostic = r.ok ? r.data.diagnostic : r.error.message;
      setLoopbackTest({
        status: "failed",
        message: diagnostic || "回环连接测试失败"
      });
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-medium mb-2">网络诊断</h3>
        <p className="text-xs text-muted-foreground mb-3">
          测试本地回环连接（127.0.0.1）是否正常。如果 VPN kill-switch 阻塞了本地流量，增强功能将无法使用。
        </p>
        <button
          onClick={testLoopback}
          disabled={busy}
          className="text-xs px-3 py-1.5 rounded border border-border hover:bg-accent disabled:opacity-50 transition-colors"
        >
          {busy ? (
            <><Loader2 className="inline size-3 animate-spin mr-1" />测试中...</>
          ) : (
            "测试本地回环连接"
          )}
        </button>
      </div>

      {loopbackTest.status && (
        <div className={`p-3 rounded text-xs ${
          loopbackTest.status === "success"
            ? "bg-green-50 text-green-900 border border-green-200"
            : "bg-red-50 text-red-900 border border-red-200"
        }`}>
          <div className="flex items-start gap-2">
            {loopbackTest.status === "success" ? (
              <CheckCircle className="size-4 flex-shrink-0 mt-0.5" />
            ) : (
              <AlertCircle className="size-4 flex-shrink-0 mt-0.5" />
            )}
            <pre className="whitespace-pre-wrap font-mono text-xs flex-1">
              {loopbackTest.message}
            </pre>
          </div>
        </div>
      )}

      <div className="pt-3 border-t">
        <h3 className="text-sm font-medium mb-2">反馈包</h3>
        <button onClick={exportPack} disabled={busy} className="text-xs px-2 py-1 rounded border border-border">
          导出反馈包到桌面
        </button>
        {path && <p className="text-xs text-muted-foreground mt-2">已导出：{path}</p>}
        {error && <p className="text-xs text-destructive mt-2">{error}</p>}
      </div>
    </div>
  );
}

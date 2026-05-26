import { useTheme, type ThemePreference } from "@/state/useTheme";

const OPTIONS: { value: ThemePreference; label: string; desc: string }[] = [
  { value: "light", label: "浅色", desc: "始终使用浅色" },
  { value: "dark", label: "深色", desc: "始终使用深色" },
  { value: "auto", label: "跟随系统", desc: "随系统/浏览器主题切换" },
];

export function ThemePanel() {
  const { preference, resolved, setPreference } = useTheme();
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">主题</h3>
        <span className="text-xs text-muted-foreground">
          当前：{resolved === "dark" ? "深色" : "浅色"}
        </span>
      </div>
      <div className="grid grid-cols-3 gap-2">
        {OPTIONS.map((opt) => {
          const active = preference === opt.value;
          return (
            <button
              key={opt.value}
              onClick={() => setPreference(opt.value)}
              className={[
                "rounded border px-2 py-2 text-left transition",
                active
                  ? "border-primary bg-primary/10"
                  : "border-border hover:bg-muted",
              ].join(" ")}
            >
              <div className="text-sm font-medium">{opt.label}</div>
              <div className="text-xs text-muted-foreground mt-1 leading-snug">
                {opt.desc}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

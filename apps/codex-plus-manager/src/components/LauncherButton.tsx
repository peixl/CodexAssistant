import { Loader2, Rocket, AlertTriangle, ArrowRight } from "lucide-react";
import type { LauncherState } from "@/state/launcherMachine";
import { TEXT } from "@/lib/text";

export function LauncherButton({
  state,
  onLaunch,
  onRetry,
  onOpenAccount,
}: {
  state: LauncherState;
  onLaunch: () => void;
  onRetry: () => void;
  onOpenAccount: () => void;
}) {
  const base =
    "w-[320px] h-[120px] rounded-2xl text-xl font-semibold flex flex-col items-center justify-center gap-2 transition";
  switch (state.kind) {
    case "ready":
      return (
        <button onClick={onLaunch} className={`${base} bg-primary text-primary-foreground hover:opacity-90`}>
          <span className="flex items-center gap-2"><Rocket className="size-6" /> {TEXT.launcher.ready}</span>
          <span className="text-sm font-normal opacity-80">{TEXT.launcher.readyHint}</span>
        </button>
      );
    case "launching":
    case "preparing":
      return (
        <button disabled className={`${base} bg-primary/80 text-primary-foreground cursor-not-allowed`}>
          <span className="flex items-center gap-2"><Loader2 className="size-6 animate-spin" />
            {state.kind === "launching" ? TEXT.launcher.launching : TEXT.launcher.preparing}
          </span>
        </button>
      );
    case "need_account":
      return (
        <button onClick={onOpenAccount} className={`${base} border border-border bg-background hover:bg-muted`}>
          <span className="flex items-center gap-2"><ArrowRight className="size-6" /> {TEXT.launcher.needAccount}</span>
          <span className="text-sm font-normal opacity-80">{TEXT.launcher.needAccountHint}</span>
        </button>
      );
    case "error":
      return (
        <button onClick={onRetry} className={`${base} border border-destructive text-destructive bg-background hover:bg-destructive/10`}>
          <span className="flex items-center gap-2"><AlertTriangle className="size-6" /> {TEXT.launcher.errorPrefix}</span>
          <span className="text-sm font-normal opacity-80">{state.message}</span>
        </button>
      );
  }
}

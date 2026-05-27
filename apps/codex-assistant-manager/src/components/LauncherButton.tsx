import { Loader2, Rocket, AlertTriangle, ArrowRight } from "lucide-react";
import type { LauncherState } from "@/state/launcherMachine";
import { TEXT } from "@/lib/text";

function compactLaunchMessage(message: string) {
  if (/loopback|localhost|127\.0\.0\.1|CDP/i.test(message)) {
    return TEXT.launcher.degradedHint;
  }
  return message;
}

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
    "launcher-button w-[min(100%,360px)] min-h-[120px] rounded-2xl text-xl font-semibold flex flex-col items-center justify-center gap-2 px-5 py-4 text-center transition";
  switch (state.kind) {
    case "ready":
      return (
        <button onClick={onLaunch} className={`${base} bg-primary text-primary-foreground hover:opacity-90`}>
          <span className="launcher-button-label"><Rocket className="size-6" /> {TEXT.launcher.ready}</span>
          <span className="text-sm font-normal opacity-80">{TEXT.launcher.readyHint}</span>
        </button>
      );
    case "launching":
    case "preparing":
      return (
        <button disabled className={`${base} bg-primary/80 text-primary-foreground cursor-not-allowed`}>
          <span className="launcher-button-label"><Loader2 className="size-6 animate-spin" />
            {state.kind === "launching" ? TEXT.launcher.launching : TEXT.launcher.preparing}
          </span>
          {state.kind === "launching" && (
            <span className="text-sm font-normal opacity-80">{TEXT.launcher.launchingHint}</span>
          )}
        </button>
      );
    case "need_account":
      return (
        <button onClick={onOpenAccount} className={`${base} border border-border bg-background hover:bg-muted`}>
          <span className="launcher-button-label"><ArrowRight className="size-6" /> {TEXT.launcher.needAccount}</span>
          <span className="text-sm font-normal opacity-80">{TEXT.launcher.needAccountHint}</span>
        </button>
      );
    case "degraded":
      return (
        <button
          onClick={onRetry}
          className={`${base} border border-amber-500/70 text-amber-700 bg-amber-50 hover:bg-amber-100 dark:text-amber-300 dark:bg-amber-950/20 dark:hover:bg-amber-950/35`}
          title={state.message}
        >
          <span className="launcher-button-label"><AlertTriangle className="size-6" /> {TEXT.launcher.degraded}</span>
          <span className="launcher-message text-sm font-normal opacity-90">{compactLaunchMessage(state.message)}</span>
        </button>
      );
    case "error":
      return (
        <button
          onClick={onRetry}
          className={`${base} border border-destructive text-destructive bg-background hover:bg-destructive/10`}
          title={state.message}
        >
          <span className="launcher-button-label"><AlertTriangle className="size-6" /> {TEXT.launcher.errorPrefix}</span>
          <span className="launcher-message text-sm font-normal opacity-80">{state.message}</span>
        </button>
      );
  }
}

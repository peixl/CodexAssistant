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

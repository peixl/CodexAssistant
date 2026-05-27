import { Settings } from "lucide-react";
import { TEXT } from "@/lib/text";
import { LauncherButton } from "@/components/LauncherButton";
import { CapabilityChips } from "@/components/CapabilityChips";
import { UpdateBanner } from "@/components/UpdateBanner";
import { AccountStatusCard } from "@/components/AccountStatusCard";
import type { LauncherState } from "@/state/launcherMachine";
import type { RelayKind } from "@/state/useLauncherMachine";
import type { UpdateInfo } from "@/state/useUpdateProbe";

export function Home({
  state,
  relayKind,
  updateInfo,
  onLaunch,
  onRetry,
  onOpenAccount,
  onOpenMore,
  onOpenUpdate,
}: {
  state: LauncherState;
  relayKind: RelayKind;
  updateInfo: UpdateInfo | null;
  onLaunch: () => void;
  onRetry: () => void;
  onOpenAccount: () => void;
  onOpenMore: () => void;
  onOpenUpdate: () => void;
}) {
  return (
    <main className="min-h-screen flex flex-col">
      <header className="flex items-start justify-between gap-4 px-6 py-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="text-base font-medium">{TEXT.appName}</h1>
            <span className="rounded-full border border-border px-2 py-0.5 text-[11px] font-semibold tracking-normal text-muted-foreground">
              {TEXT.brand.mark}
            </span>
          </div>
          <p className="mt-1 max-w-[460px] truncate text-xs text-muted-foreground">{TEXT.brand.manifesto}</p>
        </div>
        <button
          onClick={onOpenMore}
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <Settings className="size-4" /> {TEXT.more.title}
        </button>
      </header>

      <section className="flex-1 flex flex-col items-center justify-center gap-6 px-6">
        <LauncherButton state={state} onLaunch={onLaunch} onRetry={onRetry} onOpenAccount={onOpenAccount} />
        <CapabilityChips />
        <div className="text-xs text-muted-foreground">{TEXT.brand.platformFocus}</div>
        <UpdateBanner info={updateInfo} onUpdate={onOpenUpdate} />
      </section>

      <footer className="flex justify-end px-6 py-4">
        <AccountStatusCard relayKind={relayKind} onClick={onOpenAccount} />
      </footer>
    </main>
  );
}

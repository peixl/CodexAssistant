import { useCallback, useMemo, useState } from "react";
import { Home } from "@/screens/Home";
import { AccountDrawer } from "@/drawers/AccountDrawer";
import { MoreDrawer } from "@/drawers/MoreDrawer";
import { useBackend } from "@/state/useBackend";
import { useLauncherMachine, type RelayKind } from "@/state/useLauncherMachine";
import { useUpdateProbe } from "@/state/useUpdateProbe";
import { callSafe } from "@/lib/invoke";
import { TEXT } from "@/lib/text";

function inferRelayKind(
  applied: boolean,
  requiresOpenaiAuth: boolean,
  authenticated: boolean,
  hasApiKey: boolean,
): RelayKind {
  if (!applied) return "none";
  if (requiresOpenaiAuth) return "apiKey";
  if (authenticated) return "chatgpt";
  return hasApiKey ? "apiKey" : "chatgpt";
}

export function App() {
  const { overview, probe, settings, refresh } = useBackend();
  const [accountOpen, setAccountOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [updateBusy, setUpdateBusy] = useState(false);

  const relayKind: RelayKind = useMemo(
    () => inferRelayKind(
      !!probe?.relayApplied,
      !!probe?.requiresOpenaiAuth,
      !!probe?.authenticated,
      !!settings?.hasApiKey,
    ),
    [probe?.relayApplied, probe?.requiresOpenaiAuth, probe?.authenticated, settings?.hasApiKey],
  );

  const launchArgs = useMemo(
    () => ({
      appPath: overview?.appPath ?? null,
      debugPort: overview?.debugPort ?? 9229,
      helperPort: overview?.helperPort ?? 57321,
    }),
    [overview?.appPath, overview?.debugPort, overview?.helperPort],
  );

  const onAfterLaunch = useCallback(async () => { await refresh(); }, [refresh]);

  const { state, launch, retry } = useLauncherMachine({
    probe,
    relayKind,
    launchArgs,
    onAfterLaunch,
  });

  const updateInfo = useUpdateProbe(5000);

  const onOpenUpdate = useCallback(async () => {
    if (!updateInfo?.available) return;
    setUpdateBusy(true);
    const release = {
      version: updateInfo.latestVersion ?? "",
      url: "",
      body: "",
      asset_name: updateInfo.assetName,
      asset_url: updateInfo.assetUrl,
      asset_sha256: updateInfo.assetSha256,
    };
    const r = await callSafe("perform_update", { release });
    setUpdateBusy(false);
    if (!r.ok) {
      alert(`${TEXT.update.failedTitle}\n\n${r.error.message}`);
    }
  }, [updateInfo]);

  return (
    <>
      <Home
        state={state}
        relayKind={relayKind}
        updateInfo={updateInfo}
        onLaunch={launch}
        onRetry={retry}
        onOpenAccount={() => setAccountOpen(true)}
        onOpenMore={() => setMoreOpen(true)}
        onOpenUpdate={onOpenUpdate}
      />
      <AccountDrawer
        open={accountOpen}
        onClose={() => setAccountOpen(false)}
        current={relayKind}
        onApplied={refresh}
      />
      <MoreDrawer open={moreOpen} onClose={() => setMoreOpen(false)} />
      {updateBusy && (
        <div className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center">
          <div className="bg-background border border-border rounded p-6 text-sm">正在下载并校验更新…</div>
        </div>
      )}
    </>
  );
}

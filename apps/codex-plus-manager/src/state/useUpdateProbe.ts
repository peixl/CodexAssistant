import { useEffect, useState } from "react";
import { callSafe } from "@/lib/invoke";

export type UpdateInfo = {
  available: boolean;
  latestVersion: string | null;
  assetUrl: string | null;
  assetSha256: string | null;
  assetName: string | null;
};

export function useUpdateProbe(delayMs = 5000): UpdateInfo | null {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  useEffect(() => {
    const timer = setTimeout(async () => {
      const r = await callSafe<Record<string, unknown>>("check_update");
      if (!r.ok) return;
      const d = r.data as {
        update_available?: boolean;
        latest_version?: string | null;
        assetUrl?: string | null;
        assetName?: string | null;
        assetSha256?: string | null;
      };
      setInfo({
        available: !!d.update_available,
        latestVersion: d.latest_version ?? null,
        assetUrl: d.assetUrl ?? null,
        assetName: d.assetName ?? null,
        assetSha256: d.assetSha256 ?? null,
      });
    }, delayMs);
    return () => clearTimeout(timer);
  }, [delayMs]);
  return info;
}

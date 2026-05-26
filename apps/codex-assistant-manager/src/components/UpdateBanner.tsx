import { TEXT } from "@/lib/text";
import type { UpdateInfo } from "@/state/useUpdateProbe";

export function UpdateBanner({
  info,
  onUpdate,
}: {
  info: UpdateInfo | null;
  onUpdate: () => void;
}) {
  if (!info?.available || !info.latestVersion) return null;
  return (
    <p className="text-sm text-muted-foreground">
      {TEXT.update.available(info.latestVersion)}{"  "}
      <button onClick={onUpdate} className="underline text-primary hover:opacity-80">
        {TEXT.update.cta}
      </button>
    </p>
  );
}

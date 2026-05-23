import { UserCircle2 } from "lucide-react";
import { TEXT } from "@/lib/text";
import type { RelayKind } from "@/state/useLauncherMachine";

export function AccountStatusCard({
  relayKind,
  onClick,
}: {
  relayKind: RelayKind;
  onClick: () => void;
}) {
  const label =
    relayKind === "apiKey" ? TEXT.account.apiKey
      : relayKind === "chatgpt" ? TEXT.account.chatgpt
      : "未配置";
  return (
    <button
      onClick={onClick}
      className="inline-flex items-center gap-2 px-4 py-2 rounded-xl border border-border hover:bg-muted text-sm"
    >
      <UserCircle2 className="size-5" />
      <span className="opacity-80">{TEXT.account.title}</span>
      <span className="font-medium">{label}</span>
    </button>
  );
}

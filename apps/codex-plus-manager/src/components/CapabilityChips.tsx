import { Check } from "lucide-react";
import { TEXT } from "@/lib/text";

const items = [
  TEXT.capabilities.plugins,
  TEXT.capabilities.deleteChats,
  TEXT.capabilities.exportMd,
  TEXT.capabilities.autoUpdate,
];

export function CapabilityChips() {
  return (
    <div className="flex flex-wrap items-center justify-center gap-3 text-sm text-muted-foreground">
      {items.map((label) => (
        <span key={label} className="inline-flex items-center gap-1">
          <Check className="size-4 text-primary" />
          {label}
        </span>
      ))}
    </div>
  );
}

import { type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { Drawer } from "@/components/Drawer";
import { TEXT } from "@/lib/text";
import { ScriptsPanel } from "@/panels/ScriptsPanel";
import { ProvidersPanel } from "@/panels/ProvidersPanel";
import { EntryPointsPanel } from "@/panels/EntryPointsPanel";
import { DiagnosticsPanel } from "@/panels/DiagnosticsPanel";
import { RelayAdvancedPanel } from "@/panels/RelayAdvancedPanel";
import { AboutPanel } from "@/panels/AboutPanel";
import { ThemePanel } from "@/panels/ThemePanel";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <details className="group border border-border rounded">
      <summary className="cursor-pointer list-none px-3 py-2 flex items-center justify-between text-sm">
        {title}
        <ChevronDown className="size-4 transition group-open:rotate-180" />
      </summary>
      <div className="px-3 py-3 border-t border-border">{children}</div>
    </details>
  );
}

export function MoreDrawer({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Drawer open={open} title={TEXT.more.title} onClose={onClose}>
      <div className="space-y-3">
        <Section title={TEXT.more.sections.appearance}><ThemePanel /></Section>
        <Section title={TEXT.more.sections.scripts}><ScriptsPanel /></Section>
        <Section title={TEXT.more.sections.providers}><ProvidersPanel /></Section>
        <Section title={TEXT.more.sections.entryPoints}><EntryPointsPanel /></Section>
        <Section title={TEXT.more.sections.diagnostics}><DiagnosticsPanel /></Section>
        <Section title={TEXT.more.sections.relayAdvanced}><RelayAdvancedPanel /></Section>
        <Section title={TEXT.more.sections.about}><AboutPanel /></Section>
      </div>
    </Drawer>
  );
}

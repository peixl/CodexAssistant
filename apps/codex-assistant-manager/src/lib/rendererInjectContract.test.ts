import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const rendererInjectPath = path.resolve(process.cwd(), "../../assets/inject/renderer-inject.js");
const rendererInject = fs.readFileSync(rendererInjectPath, "utf8");

describe("renderer injection contract", () => {
  it("keeps the IFQ.AI brand entry visible in the injected settings UI", () => {
    expect(rendererInject).toContain("捷时云服务 - by IFQ.AI");
    expect(rendererInject).toContain("https://cloud.ifq.ai");
    expect(rendererInject).toContain("IFQ.AI</strong>");
  });

  it("keeps plugin unlock controls and runtime hooks enabled outside relay mode", () => {
    expect(rendererInject).toContain("pluginEntryUnlock: true");
    expect(rendererInject).toContain("forcePluginInstall: true");
    expect(rendererInject).toContain('data-codex-assistant-setting="pluginEntryUnlock"');
    expect(rendererInject).toContain("function enablePluginEntry()");
    expect(rendererInject).toContain("function unblockPluginInstallButtons()");
    expect(rendererInject).toContain("Plugins - Unlocked");
  });

  it("keeps session delete and Markdown export actions wired to bridge routes", () => {
    expect(rendererInject).toContain("sessionDelete: true");
    expect(rendererInject).toContain("markdownExport: true");
    expect(rendererInject).toContain('postJson("/delete", ref)');
    expect(rendererInject).toContain('postJson("/export-markdown", ref)');
    expect(rendererInject).toContain('new Blob([markdown], { type: "text/markdown;charset=utf-8" })');
    expect(rendererInject).toContain("function installDeleteButtonEventDelegation()");
  });
});

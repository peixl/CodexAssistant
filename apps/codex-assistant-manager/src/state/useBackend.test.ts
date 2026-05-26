import { describe, expect, it } from "vitest";
import { extractActiveProfileForTest, mergeApiKeyForTest } from "./useBackend";

describe("extractActiveProfile", () => {
  it("returns active profile fields trimmed", () => {
    expect(
      extractActiveProfileForTest({
        activeRelayId: "p2",
        relayProfiles: [
          { id: "p1", apiKey: " skipped ", baseUrl: " base1 " },
          { id: "p2", apiKey: "  sk-real  ", baseUrl: "  https://api.example.com  " },
        ],
      }),
    ).toEqual({
      apiKey: "sk-real",
      baseUrl: "https://api.example.com",
      activeRelayId: "p2",
    });
  });

  it("falls back to first profile when activeRelayId missing", () => {
    expect(
      extractActiveProfileForTest({
        relayProfiles: [{ id: "p1", apiKey: "sk-x", baseUrl: "https://x" }],
      }),
    ).toEqual({ apiKey: "sk-x", baseUrl: "https://x", activeRelayId: "" });
  });

  it("returns empty strings for missing data", () => {
    expect(extractActiveProfileForTest({})).toEqual({
      apiKey: "",
      baseUrl: "",
      activeRelayId: "",
    });
  });
});

describe("mergeApiKeyIntoSettings", () => {
  it("updates apiKey on the active profile and preserves other fields", () => {
    const merged = mergeApiKeyForTest(
      {
        codexAppPath: "C:/x",
        activeRelayId: "p1",
        relayProfiles: [
          { id: "p1", apiKey: "old", baseUrl: "https://old", protocol: "responses" },
        ],
      },
      "sk-new",
      "https://new",
    );
    expect((merged as { codexAppPath: string }).codexAppPath).toBe("C:/x");
    expect((merged as { relayProfiles: { id: string; apiKey: string; baseUrl: string; protocol?: string }[] }).relayProfiles[0]).toEqual({
      id: "p1",
      apiKey: "sk-new",
      baseUrl: "https://new",
      protocol: "responses",
    });
  });

  it("only updates apiKey when baseUrl is empty", () => {
    const merged = mergeApiKeyForTest(
      {
        activeRelayId: "p1",
        relayProfiles: [{ id: "p1", apiKey: "old", baseUrl: "https://keep" }],
      },
      "sk-new",
      "",
    );
    const profile = (merged as { relayProfiles: { apiKey: string; baseUrl: string }[] }).relayProfiles[0];
    expect(profile.apiKey).toBe("sk-new");
    expect(profile.baseUrl).toBe("https://keep");
  });

  it("creates a profile when none exist", () => {
    const merged = mergeApiKeyForTest({ activeRelayId: "default" }, "sk-x", "https://x");
    const profiles = (merged as { relayProfiles: { id: string; apiKey: string; baseUrl: string }[] }).relayProfiles;
    expect(profiles).toHaveLength(1);
    expect(profiles[0]).toEqual({ id: "default", apiKey: "sk-x", baseUrl: "https://x" });
  });
});

import { describe, expect, it } from "vitest";
import { groupOfSetting, searchSettings, SETTING_ENTRIES, SETTINGS_SECTIONS } from "./searchIndex";

describe("searchSettings", () => {
  it("finds a setting by its name", () => {
    const [first] = searchSettings("main model");
    expect(first).toMatchObject({ tab: "models", key: "model.main", title: "Main model" });
  });

  it("finds a setting by what people call it", () => {
    expect(searchSettings("budget").map((e) => e.key)).toContain("agents.session_cost_cap_usd");
    expect(searchSettings("api key")[0]?.tab).toBe("providers");
  });

  it("finds the pet under its old name too", () => {
    expect(searchSettings("companion").every((e) => e.tab === "pet")).toBe(true);
    expect(searchSettings("roam")[0]?.key).toBe("ui.pet.roam");
  });

  it("ranks titles that start with the words first", () => {
    expect(searchSettings("compact")[0]?.key).toBe("context.compact_at_percent");
  });

  it("needs every word to match and ignores an empty query", () => {
    expect(searchSettings("model zzz")).toEqual([]);
    expect(searchSettings("  ")).toEqual([]);
  });
});

describe("the index", () => {
  it("files every entry under a tab of a section", () => {
    const tabs = new Set(SETTINGS_SECTIONS.flatMap((s) => s.tabs));
    expect(SETTING_ENTRIES.every((e) => tabs.has(e.tab))).toBe(true);
    expect(new Set(SETTING_ENTRIES.map((e) => e.key)).size).toBe(SETTING_ENTRIES.length);
  });

  it("knows which group holds a setting, so a folded group can open", () => {
    expect(groupOfSetting("web.search_backend")).toBe("Web");
    expect(groupOfSetting("nope")).toBeNull();
  });
});

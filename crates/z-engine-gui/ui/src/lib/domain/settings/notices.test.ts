import { describe, expect, it } from "vitest";
import type { LoadedSettings } from "../../protocol/config/LoadedSettings";
import type { Settings } from "../../protocol/config/Settings";
import { settingsNotices } from "./notices";

const note = "imported from the v1 file /p/.z-engine/config.toml in memory; saved to /p/.z-engine/settings.toml on the first change";

const loaded: LoadedSettings = {
  settings: {} as Settings,
  layers: [
    { scope: "default", path: null, exists: true, error: null, note: null },
    { scope: "user", path: "/c/settings.toml", exists: true, error: "unknown effort `extreme`", note: null },
    { scope: "project", path: "/p/.z-engine/config.toml", exists: true, error: null, note },
    { scope: "env", path: null, exists: true, error: "bad kind", note: null },
  ],
  warnings: [
    "/c/settings.toml was skipped: unknown effort `extreme`",
    note,
    "/p/.z-engine/config.toml: `review` was removed",
    "environment overrides were skipped: bad kind",
    "model.max_output_tokens = 10 is out of range; using 256",
  ],
};

describe("settingsNotices", () => {
  it("shows skipped layers, import notes and the remaining warnings once each", () => {
    expect(settingsNotices(loaded)).toEqual([
      { tone: "error", text: "User settings (/c/settings.toml) were skipped: unknown effort `extreme`" },
      { tone: "error", text: "Environment settings were skipped: bad kind" },
      { tone: "info", text: note },
      { tone: "warn", text: "/p/.z-engine/config.toml: `review` was removed" },
      { tone: "warn", text: "model.max_output_tokens = 10 is out of range; using 256" },
    ]);
  });

  it("is empty before settings load", () => {
    expect(settingsNotices(null)).toEqual([]);
  });
});

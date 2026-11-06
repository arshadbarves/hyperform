import { describe, expect, it } from "vitest";
import { APPEARANCE_OPTIONS } from "./appearanceSettings";

describe("appearance settings", () => {
  it("offers the three report density modes in increasing detail", () => {
    expect(APPEARANCE_OPTIONS.map(({ value, label }) => ({ value, label }))).toEqual([
      { value: "quiet", label: "Quiet" },
      { value: "compact", label: "Compact" },
      { value: "detailed", label: "Detailed" },
    ]);
    expect(APPEARANCE_OPTIONS.every((option) => option.description.length > 20)).toBe(true);
  });
});

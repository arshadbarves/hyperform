import { describe, expect, it } from "vitest";
import {
  DEFAULT_PET_NAME,
  isPetAccessory,
  PET_LOOKS,
  PET_NAME_MAX,
  petName,
  petStage,
  unlockedAccessories,
  unlockedTricks,
  unlocksBetween,
} from "./looks";

describe("pet looks", () => {
  it("offers six looks with unique ids", () => {
    expect(new Set(PET_LOOKS.map((l) => l.id)).size).toBe(6);
  });

  it("keeps a name short, trimmed and never empty", () => {
    expect(petName("  Pip ")).toBe("Pip");
    expect(petName("")).toBe(DEFAULT_PET_NAME);
    expect(petName(null)).toBe(DEFAULT_PET_NAME);
    expect(Array.from(petName("🐱".repeat(40)))).toHaveLength(PET_NAME_MAX);
  });

  it("grows through four stages", () => {
    expect(petStage(1)).toBe("seed");
    expect(petStage(2)).toBe("seed");
    expect(petStage(3)).toBe("sprout");
    expect(petStage(6)).toBe("bloom");
    expect(petStage(10)).toBe("star");
    expect(petStage(40)).toBe("star");
  });

  it("unlocks accessories and tricks by level", () => {
    expect(unlockedAccessories(1)).toEqual([]);
    expect(unlockedAccessories(4)).toEqual(["sprout", "scarf"]);
    expect(unlockedTricks(1)).toEqual(["stretch"]);
    expect(unlockedTricks(7)).toEqual(["stretch", "hop", "spin", "sparkle"]);
    expect(unlocksBetween(1, 3)).toEqual({ accessories: ["sprout"], tricks: ["hop"] });
    expect(unlocksBetween(5, 5)).toEqual({ accessories: [], tricks: [] });
  });

  it("recognizes accessories", () => {
    expect(isPetAccessory("scarf")).toBe(true);
    expect(isPetAccessory("hat")).toBe(false);
    expect(isPetAccessory(null)).toBe(false);
  });
});

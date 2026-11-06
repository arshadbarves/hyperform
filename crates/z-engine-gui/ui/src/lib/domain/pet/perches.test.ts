import { describe, expect, it } from "vitest";
import { alongFor, nearestPerch, PERCH_SIZE, spotOn, usable, type PerchRect } from "./perches";

const composer: PerchRect = { id: "composer", kind: "edge", left: 300, top: 600, width: 680, height: 110 };
const island: PerchRect = { id: "island", kind: "slot", left: 500, top: 8, width: 36, height: 36 };
const sidebar: PerchRect = { id: "sidebar", kind: "edge", left: 8, top: 700, width: 260, height: 48 };

describe("perches", () => {
  it("centers the pet in a slot, feet at the bottom of its box", () => {
    expect(spotOn(island, 0)).toEqual({ x: 518, y: 26 + PERCH_SIZE.island / 2, size: 28 });
  });

  it("stands the pet on an edge, inset from both ends", () => {
    const size = PERCH_SIZE.composer;
    expect(spotOn(composer, 0)).toEqual({ x: 300 + 18 + size / 2, y: 600, size });
    expect(spotOn(composer, 1).x).toBe(300 + 680 - 18 - size / 2);
    expect(spotOn(composer, 5).x).toBe(spotOn(composer, 1).x);
  });

  it("maps a point back to the nearest place along an edge", () => {
    expect(alongFor(composer, 0)).toBe(0);
    expect(alongFor(composer, 10_000)).toBe(1);
    expect(alongFor(composer, spotOn(composer, 0.4).x)).toBeCloseTo(0.4);
    expect(alongFor(island, 0)).toBe(0.5);
  });

  it("drops the pet on the closest perch", () => {
    expect(nearestPerch([composer, island, sidebar], 520, 30)?.id).toBe("island");
    expect(nearestPerch([composer, island, sidebar], 700, 560)?.id).toBe("composer");
    expect(nearestPerch([composer, island, sidebar], 60, 690)?.id).toBe("sidebar");
    expect(nearestPerch([], 0, 0)).toBeNull();
  });

  it("skips perches that are off screen or too small", () => {
    const viewport = { width: 1200, height: 800 };
    expect(usable(composer, viewport)).toBe(true);
    expect(usable({ ...composer, top: 900 }, viewport)).toBe(false);
    expect(usable({ ...composer, width: 60 }, viewport)).toBe(false);
    expect(usable({ ...island, width: 0 }, viewport)).toBe(false);
  });

  it("needs a slot roomy enough for the pet, like the panel band's free space", () => {
    const viewport = { width: 1200, height: 800 };
    const band: PerchRect = { id: "panel", kind: "slot", left: 1000, top: 8, width: 60, height: 40 };
    expect(usable(band, viewport)).toBe(true);
    expect(usable({ ...band, width: 12 }, viewport)).toBe(false);
    expect(usable({ ...island, width: 24, height: 24 }, viewport)).toBe(true);
  });
});

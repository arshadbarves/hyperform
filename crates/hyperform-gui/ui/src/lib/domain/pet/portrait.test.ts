import { describe, expect, it } from "vitest";
import { PORTRAIT_FRAME, PORTRAIT_PITCH, PORTRAIT_YAW, portraitFor, portraitFrame, portraitLook } from "./portrait";
import { SEED_SCALE } from "./rig3d";

const slot = { x: 600, y: 22 };

describe("portraitLook", () => {
  it("turns toward the pet's side, and looks at it", () => {
    const left = portraitLook(slot, { x: 480, y: 22 });
    const right = portraitLook(slot, { x: 720, y: 22 });
    expect(left.yaw).toBeLessThan(0);
    expect(right.yaw).toBeGreaterThan(0);
    expect(right.yaw).toBeCloseTo(-left.yaw);
    expect(left.pitch).toBe(0);
    expect(right.lookAt).toEqual({ x: 720, y: 22 });
  });

  it("tips its head down toward a pet below, turning less for a pet nearby", () => {
    const below = portraitLook(slot, { x: 600, y: 90 });
    expect(below.pitch).toBeGreaterThan(0);
    expect(below.yaw).toBe(0);
    expect(portraitLook(slot, { x: 640, y: 22 }).yaw).toBeLessThan(portraitLook(slot, { x: 760, y: 22 }).yaw);
    expect(portraitLook(slot, { x: 600, y: 0 }).pitch).toBeLessThan(0);
  });

  it("turns only so far, and less on each axis toward a corner", () => {
    expect(portraitLook(slot, { x: 9000, y: 22 }).yaw).toBeCloseTo(PORTRAIT_YAW);
    expect(portraitLook(slot, { x: 600, y: -9000 }).pitch).toBeCloseTo(-PORTRAIT_PITCH);
    const far = portraitLook(slot, { x: -4000, y: 5000 });
    expect(far.yaw).toBeLessThan(0);
    expect(far.pitch).toBeGreaterThan(0);
    expect(Math.hypot(far.yaw / PORTRAIT_YAW, far.pitch / PORTRAIT_PITCH)).toBeCloseTo(1);
    const near = portraitLook(slot, { x: 640, y: 40 });
    expect(Math.hypot(near.yaw / PORTRAIT_YAW, near.pitch / PORTRAIT_PITCH)).toBeLessThan(1);
    expect(near.yaw).toBeCloseTo(Math.atan2(40, 320));
  });

  it("faces forward with no pet to look at", () => {
    expect(portraitLook(slot, null)).toEqual({ yaw: 0, pitch: 0, lookAt: null });
    expect(portraitLook(slot, { x: Number.NaN, y: 40 }).yaw).toBe(0);
  });
});

describe("portraitFrame", () => {
  const rest = { x: 0, y: 0, rotZ: 0, sx: 1, sy: 1 };

  it("is the close-up itself for a body at rest", () => {
    expect(portraitFrame(rest)).toEqual({ ...PORTRAIT_FRAME });
  });

  it("goes down with a body that sinks or slumps, and less for a squash than a drop", () => {
    expect(portraitFrame({ ...rest, y: 5 }).cy).toBeCloseTo(PORTRAIT_FRAME.cy + 5);
    const slump = portraitFrame({ ...rest, y: 1, sx: 1.05, sy: 0.92 });
    expect(slump.cy).toBeCloseTo(PORTRAIT_FRAME.cy + 1 + (28 - PORTRAIT_FRAME.cy) * 0.08);
    expect(slump.cx).toBe(16);
  });

  it("leans with the body about its feet", () => {
    const lean = portraitFrame({ ...rest, x: 0.6, rotZ: (7 * Math.PI) / 180 });
    expect(lean.cx).toBeCloseTo(16 + 0.6 + (28 - PORTRAIT_FRAME.cy) * Math.sin((7 * Math.PI) / 180));
    expect(lean.cy).toBeGreaterThan(PORTRAIT_FRAME.cy);
  });

  it("closes in on the seed's smaller face as much as the grown one's", () => {
    const seed = portraitFrame(rest, 0.86);
    expect(seed.span).toBeCloseTo(PORTRAIT_FRAME.span * 0.86);
    expect(28 - seed.cy).toBeCloseTo((28 - PORTRAIT_FRAME.cy) * 0.86);
  });

  it("frames a mood's held body and a stage, not its loop", () => {
    expect(portraitFor("sink", null, "seed")).toEqual(portraitFrame({ ...rest, y: 5 }, SEED_SCALE));
    expect(portraitFor("bounce", null, "sprout")).toEqual(portraitFrame(rest));
    expect(portraitFor("breathe", "lieDown", "sprout").cy).toBeGreaterThan(PORTRAIT_FRAME.cy);
  });
});

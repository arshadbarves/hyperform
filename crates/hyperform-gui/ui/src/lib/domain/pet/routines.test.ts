import { describe, expect, it } from "vitest";
import { DROWSY_MS, ROUTINE_MS, blinkGap, blinkPattern, gazeToward, pickRoutine, type PetRoutine } from "./routines";

function spread(tricks: Parameters<typeof pickRoutine>[0]["tricks"], idleMs: number): Set<PetRoutine> {
  const seen = new Set<PetRoutine>();
  for (let i = 0; i < 100; i++) seen.add(pickRoutine({ tricks, idleMs, random: i / 100 }));
  return seen;
}

describe("pickRoutine", () => {
  it("looks around, sits, taps and yawns, and plays the tricks it has unlocked", () => {
    const awake = spread(["stretch", "hop"], 0);
    expect(awake).toEqual(new Set(["lookAround", "sit", "tap", "yawn", "stretch", "hop"]));
    expect(spread([], 0).has("spin")).toBe(false);
  });

  it("yawns and lies down once it has been idle a while", () => {
    const drowsy = spread(["stretch", "spin"], DROWSY_MS);
    expect(drowsy.has("lieDown")).toBe(true);
    expect(drowsy.has("spin")).toBe(false);
    expect(pickRoutine({ tricks: [], idleMs: DROWSY_MS, random: 0 })).toBe("yawn");
  });

  it("gives every routine a length", () => {
    for (const r of spread(["stretch", "hop", "spin", "sparkle"], 0)) expect(ROUTINE_MS[r]).toBeGreaterThan(0);
  });
});

describe("blinks", () => {
  it("blinks mostly once, sometimes twice or slowly", () => {
    expect(blinkPattern(0.1)).toHaveLength(1);
    expect(blinkPattern(0.7)).toHaveLength(3);
    expect(blinkPattern(0.95)[0]).toBeGreaterThan(blinkPattern(0.1)[0]);
    expect(blinkGap(0)).toBeLessThan(blinkGap(0.99));
  });
});

describe("gazeToward", () => {
  it("looks toward the point, reaching the edge of the eye when it is far", () => {
    const far = gazeToward({ x: 0, y: 0 }, { x: 500, y: 0 }, 40);
    expect(far.x).toBeCloseTo(2);
    expect(far.y).toBeCloseTo(0);
    const near = gazeToward({ x: 0, y: 0 }, { x: 10, y: 0 }, 40);
    expect(near.x).toBeLessThan(1);
  });

  it("undoes a mirrored body", () => {
    expect(gazeToward({ x: 0, y: 0 }, { x: 500, y: 0 }, 40, -1).x).toBeCloseTo(-2);
  });
});

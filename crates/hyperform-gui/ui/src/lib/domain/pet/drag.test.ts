import { describe, expect, it } from "vitest";
import { DIZZY_SWINGS, MAX_THROW, drawnFeet, grab, letGo, pull, stepHeld, type Held } from "./drag";

function hold(frames: number, move: (i: number) => number): Held {
  let held = grab({ x: 100, y: 100 }, { x: 100, y: 80 }, 0);
  for (let i = 1; i <= frames; i++) {
    held = pull(held, { x: 100 + move(i), y: 80 }, i * 16);
    held = stepHeld(held, 16, i * 16, 40).held;
  }
  return held;
}

describe("grab and pull", () => {
  it("keeps the grip where it was picked up", () => {
    const held = pull(grab({ x: 100, y: 100 }, { x: 110, y: 80 }, 0), { x: 210, y: 180 }, 16);
    expect(held.offset).toEqual({ x: 10, y: -20 });
    expect(held.aim).toEqual({ x: 200, y: 200 });
  });

  it("remembers only recent pointer samples", () => {
    let held = grab({ x: 0, y: 0 }, { x: 0, y: 0 }, 0);
    for (let i = 1; i < 40; i++) held = pull(held, { x: i, y: 0 }, i);
    expect(held.samples.length).toBeLessThanOrEqual(12);
    expect(held.samples.at(-1)?.x).toBe(39);
  });
});

describe("stepHeld", () => {
  it("follows the pointer and settles under it", () => {
    let held = pull(grab({ x: 100, y: 100 }, { x: 100, y: 80 }, 0), { x: 160, y: 80 }, 0);
    for (let i = 0; i < 90; i++) held = stepHeld(held, 16, i * 16, 40).held;
    expect(held.body.x).toBeCloseTo(160, 0);
    expect(held.body.y).toBeCloseTo(100, 0);
  });

  it("swings its feet back when the hand moves off sideways, and faces the way it is carried", () => {
    let held = pull(grab({ x: 100, y: 100 }, { x: 100, y: 80 }, 0), { x: 220, y: 80 }, 0);
    const first = stepHeld(held, 16, 16, 40);
    held = first.held;
    const next = stepHeld(held, 16, 32, 40);
    expect(next.held.swing.angle).toBeLessThan(0);
    expect(next.frame.rot).toBeGreaterThan(0);
    expect(next.frame.facing).toBe(1);
  });

  it("hangs stretched with its feet kicking", () => {
    const { frame } = stepHeld(grab({ x: 0, y: 0 }, { x: 0, y: -20 }, 0), 16, 16, 40);
    expect(frame.sy).toBeGreaterThan(1);
    expect(frame.sx).toBeLessThan(1);
    expect(frame.lift[0]).not.toBe(frame.lift[1]);
  });
});

describe("drawnFeet", () => {
  it("is the feet themselves when it hangs straight", () => {
    expect(drawnFeet({ x: 50, y: 60 }, { x: 5, y: -20 }, 0, 1, 1)).toEqual({ x: 50, y: 60 });
  });

  it("swings the feet about the grip", () => {
    const feet = drawnFeet({ x: 0, y: 0 }, { x: 0, y: -20 }, 90, 1, 1);
    expect(feet.x).toBeCloseTo(-20);
    expect(feet.y).toBeCloseTo(-20);
  });

  it("stretches them away from the grip", () => {
    expect(drawnFeet({ x: 0, y: 0 }, { x: 0, y: -20 }, 0, 1, 1.1).y).toBeCloseTo(2);
  });
});

describe("letGo", () => {
  it("keeps a throw's speed, capped", () => {
    const held = hold(6, (i) => i * 80);
    const thrown = letGo(held, 6 * 16);
    expect(thrown.v.x).toBeGreaterThan(0);
    expect(thrown.speed).toBeCloseTo(MAX_THROW);
    expect(thrown.dizzy).toBe(true);
  });

  it("drops still when the pointer stopped", () => {
    const thrown = letGo(hold(4, () => 0), 4 * 16);
    expect(thrown.speed).toBe(0);
    expect(thrown.dizzy).toBe(false);
  });

  it("leaves it dizzy after enough wild swings", () => {
    const held = { ...hold(1, () => 0), count: { side: 1 as const, wild: DIZZY_SWINGS } };
    expect(letGo(held, 16).dizzy).toBe(true);
  });
});

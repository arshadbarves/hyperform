import { describe, expect, it } from "vitest";
import {
  MAX_SWING,
  PET_SPRINGS,
  atRest,
  capSpeed,
  countSwing,
  releaseVelocity,
  stepPendulum,
  stepSpring,
  tune,
  type Body2D,
} from "./physics";

describe("stepSpring", () => {
  it("carries the body to its target and settles there", () => {
    let body: Body2D = { x: 0, y: 0, vx: 0, vy: 0 };
    const target = { x: 100, y: -40 };
    for (let i = 0; i < 120; i++) body = stepSpring(body, target, PET_SPRINGS.follow, 16);
    expect(atRest(body, target)).toBe(true);
  });

  it("keeps moving the way it was going for a moment", () => {
    const body = stepSpring({ x: 0, y: 0, vx: 1, vy: 0 }, { x: 0, y: 0 }, PET_SPRINGS.settle, 16);
    expect(body.x).toBeGreaterThan(5);
  });

  it("gives the same result for one long frame as for short ones", () => {
    const one = stepSpring({ x: 0, y: 0, vx: 0, vy: 0 }, { x: 50, y: 0 }, PET_SPRINGS.follow, 32);
    const two = stepSpring(stepSpring({ x: 0, y: 0, vx: 0, vy: 0 }, { x: 50, y: 0 }, PET_SPRINGS.follow, 16), { x: 50, y: 0 }, PET_SPRINGS.follow, 16);
    expect(one.x).toBeCloseTo(two.x, 5);
  });

  it("tunes springier springs with less damping", () => {
    expect(tune(300, 0.5).damping).toBeLessThan(tune(300, 0).damping);
    expect(tune(150).stiffness).toBeGreaterThan(tune(300).stiffness);
  });
});

describe("stepPendulum", () => {
  it("swings its feet the other way when the hand moves", () => {
    const p = stepPendulum({ angle: 0, speed: 0 }, 0.01, 40, 16);
    expect(p.angle).toBeLessThan(0);
  });

  it("comes back to hanging straight, and never swings too far", () => {
    let p = { angle: 1, speed: 0 };
    for (let i = 0; i < 400; i++) p = stepPendulum(p, 0, 40, 16);
    expect(Math.abs(p.angle)).toBeLessThan(0.05);
    const wild = stepPendulum({ angle: 0, speed: 0 }, 2, 40, 64);
    expect(Math.abs(wild.angle)).toBeLessThanOrEqual(MAX_SWING);
  });

  it("counts wild swings from side to side", () => {
    let count = { side: 0 as const, wild: 0 } as ReturnType<typeof countSwing>;
    for (const angle of [0.2, 0.7, 0.1, -0.7, 0.8, -0.9, -0.95]) count = countSwing(count, angle);
    expect(count.wild).toBe(3);
  });
});

describe("releaseVelocity", () => {
  it("measures the pointer over the last moments only", () => {
    const samples = [
      { x: 0, y: 0, t: 0 },
      { x: 10, y: 0, t: 900 },
      { x: 30, y: 10, t: 920 },
      { x: 50, y: 20, t: 940 },
    ];
    expect(releaseVelocity(samples, 950)).toEqual({ x: 1, y: 0.5 });
  });

  it("is zero when the pointer stopped before letting go", () => {
    expect(releaseVelocity([{ x: 0, y: 0, t: 0 }, { x: 10, y: 0, t: 10 }], 500)).toEqual({ x: 0, y: 0 });
  });

  it("caps the speed but keeps the direction", () => {
    const v = capSpeed({ x: 6, y: 8 }, 2);
    expect(Math.hypot(v.x, v.y)).toBeCloseTo(2);
    expect(v.x / v.y).toBeCloseTo(0.75);
  });
});

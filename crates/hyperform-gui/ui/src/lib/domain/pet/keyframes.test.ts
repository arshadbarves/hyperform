import { describe, expect, it } from "vitest";
import { EASE, cubicBezier, phase, sampleTrack } from "./keyframes";

describe("cubicBezier", () => {
  it("runs from 0 to 1, and a linear curve stays linear", () => {
    const straight = cubicBezier(0.25, 0.25, 0.75, 0.75);
    for (const u of [0, 0.1, 0.5, 0.9, 1]) expect(straight(u)).toBeCloseTo(u, 5);
    for (const ease of Object.values(EASE)) {
      expect(ease(0)).toBe(0);
      expect(ease(1)).toBe(1);
    }
  });

  it("eases in and out symmetrically, and a spring overshoots", () => {
    expect(EASE.inOut(0.5)).toBeCloseTo(0.5, 5);
    expect(EASE.inOut(0.2)).toBeLessThan(0.2);
    expect(EASE.out(0.2)).toBeGreaterThan(0.5);
    expect(EASE.in(0.3)).toBeLessThan(0.3);
    const bouncy = Array.from({ length: 99 }, (_, i) => EASE.bouncy((i + 1) / 100));
    expect(Math.max(...bouncy)).toBeGreaterThan(1.02);
  });
});

describe("sampleTrack", () => {
  const stops = [[0, 0], [0.25, -4], [0.75, 4], [1, 0]] as const;

  it("hits every stop and holds outside them", () => {
    for (const [at, value] of stops) expect(sampleTrack(stops, at, EASE.inOut)).toBeCloseTo(value);
    expect(sampleTrack(stops, -1, EASE.linear)).toBe(0);
    expect(sampleTrack(stops, 2, EASE.linear)).toBe(0);
  });

  it("eases each interval on its own", () => {
    expect(sampleTrack(stops, 0.5, EASE.inOut)).toBeCloseTo(0);
    expect(sampleTrack(stops, 0.125, EASE.linear)).toBeCloseTo(-2);
    expect(Math.abs(sampleTrack(stops, 0.125, EASE.inOut))).toBeLessThan(2);
  });
});

describe("phase", () => {
  it("loops forever, starting over each period", () => {
    expect(phase(0, 1000)).toBe(0);
    expect(phase(250, 1000)).toBeCloseTo(0.25);
    expect(phase(1000, 1000)).toBe(0);
    expect(phase(12_500, 1000)).toBeCloseTo(0.5);
  });

  it("ends a one-shot after its iterations, and waits at 0 before it starts", () => {
    expect(phase(1300, 1200, 2)).toBeCloseTo(100 / 1200);
    expect(phase(2400, 1200, 2)).toBeNull();
    expect(phase(-50, 1000, 1)).toBe(0);
    expect(phase(Number.NaN, 1000)).toBe(0);
  });
});

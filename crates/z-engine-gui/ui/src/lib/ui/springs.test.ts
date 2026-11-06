/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  SETTLE_EPSILON,
  SPRING_PRESETS,
  settleSeconds,
  spring,
  springCssVars,
  springEasing,
  springPhysics,
  springPosition,
  springs,
} from "./springs";

function points(easing: string): number[] {
  const inner = /^linear\((.*)\)$/.exec(easing)?.[1] ?? "";
  return inner.split(",").map((p) => Number(p.trim()));
}

describe("springPhysics", () => {
  it("passes physical springs through, defaulting the mass", () => {
    expect(springPhysics({ stiffness: 170, damping: 26 })).toEqual({ stiffness: 170, damping: 26, mass: 1 });
    expect(springPhysics({ stiffness: 100, damping: 10, mass: 2 }).mass).toBe(2);
  });

  it("turns response and bounce into stiffness and damping", () => {
    const { stiffness, damping, mass } = springPhysics({ response: 0.5, bounce: 0 });
    const omega = (2 * Math.PI) / 0.5;
    expect(mass).toBe(1);
    expect(stiffness).toBeCloseTo(omega * omega, 6);
    expect(damping).toBeCloseTo(2 * omega, 6);
  });

  it("damps less as bounce rises and more below zero", () => {
    const critical = springPhysics({ response: 0.4 }).damping;
    expect(springPhysics({ response: 0.4, bounce: 0.3 }).damping).toBeLessThan(critical);
    expect(springPhysics({ response: 0.4, bounce: -0.3 }).damping).toBeGreaterThan(critical);
  });
});

describe("springPosition", () => {
  it("starts at rest at 0 and ends at 1 for every damping regime", () => {
    for (const bounce of [-0.4, 0, 0.3]) {
      const x = springPosition({ response: 0.3, bounce });
      expect(x(0)).toBeCloseTo(0, 9);
      expect(x(5)).toBeCloseTo(1, 6);
    }
  });

  it("overshoots only when underdamped", () => {
    const peak = (bounce: number) => {
      const x = springPosition({ response: 0.3, bounce });
      let max = 0;
      for (let t = 0; t < 2; t += 0.001) max = Math.max(max, x(t));
      return max;
    };
    expect(peak(0.35)).toBeGreaterThan(1.02);
    expect(peak(0)).toBeLessThanOrEqual(1);
    expect(peak(-0.3)).toBeLessThanOrEqual(1);
  });
});

describe("settleSeconds", () => {
  it("is the moment the spring stays within epsilon of its target", () => {
    const params = { response: 0.3, bounce: 0.2 };
    const settled = settleSeconds(params);
    const x = springPosition(params);
    for (let t = settled; t < settled + 1; t += 0.005) expect(Math.abs(1 - x(t))).toBeLessThanOrEqual(SETTLE_EPSILON);
    expect(Math.abs(1 - x(settled - 0.01))).toBeGreaterThan(SETTLE_EPSILON * 0.5);
  });

  it("takes longer for a slower or bouncier spring", () => {
    expect(settleSeconds({ response: 0.5 })).toBeGreaterThan(settleSeconds({ response: 0.3 }));
    expect(settleSeconds({ response: 0.3, bounce: 0.5 })).toBeGreaterThan(settleSeconds({ response: 0.3, bounce: 0.1 }));
  });

  it("caps an undamped spring at ten seconds", () => {
    expect(settleSeconds({ stiffness: 100, damping: 0 })).toBe(10);
  });
});

describe("springEasing", () => {
  it("is a linear() from 0 to 1 with about one point per frame", () => {
    const easing = springEasing({ response: 0.3 }, 0.5);
    const values = points(easing);
    expect(values[0]).toBe(0);
    expect(values.at(-1)).toBe(1);
    expect(values).toHaveLength(31);
    expect(values.every((v) => Number.isFinite(v))).toBe(true);
  });

  it("keeps a minimum resolution for short springs", () => {
    expect(points(springEasing({ response: 0.05 }, 0.05))).toHaveLength(13);
  });

  it("carries the overshoot of a bouncy spring", () => {
    expect(Math.max(...points(springs.bouncy.easing))).toBeGreaterThan(1.03);
    expect(Math.max(...points(springs.smooth.easing))).toBeLessThanOrEqual(1);
  });
});

describe("spring", () => {
  it("pairs the easing with its settle duration and a sampler", () => {
    const curve = spring(SPRING_PRESETS.smooth);
    expect(curve.duration).toBe(Math.ceil(settleSeconds(SPRING_PRESETS.smooth) * 1000));
    expect(curve.at(0)).toBe(0);
    expect(curve.at(1)).toBe(1);
    expect(curve.at(0.5)).toBeGreaterThan(0.9);
  });

  it("keeps the presets within the motion budget", () => {
    for (const curve of Object.values(springs)) {
      expect(curve.duration).toBeGreaterThanOrEqual(250);
      expect(curve.duration).toBeLessThanOrEqual(800);
    }
    expect(springs.snappy.duration).toBeLessThan(springs.gentle.duration);
  });
});

describe("motion.css", () => {
  it("holds the presets exactly as springs.ts prints them", () => {
    const css = readFileSync(new URL("../../styles/motion.css", import.meta.url), "utf8")
      .replace(/\s+/g, " ")
      .replace(/\(\s+/g, "(")
      .replace(/\s+\)/g, ")");
    for (const line of springCssVars()) expect(css).toContain(line);
  });
});

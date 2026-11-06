import { afterEach, describe, expect, it, vi } from "vitest";
import type { TransitionConfig } from "svelte/transition";
import { EXIT_MS, appear, motionMs, parseCssTime, sheet, splitOff, springEasingCss, staggerMs } from "./motion";
import { springs } from "./springs";

const node = {} as Element;

function resolve(config: TransitionConfig | ((o?: { direction?: "in" | "out" }) => TransitionConfig), direction: "in" | "out") {
  return typeof config === "function" ? config({ direction }) : config;
}

afterEach(() => vi.unstubAllGlobals());

describe("motion tokens", () => {
  it("parses CSS times in either unit", () => {
    expect(parseCssTime("180ms")).toBe(180);
    expect(parseCssTime(" 1.6s ")).toBe(1600);
    expect(parseCssTime(".5s")).toBe(500);
    expect(parseCssTime("")).toBeNull();
    expect(parseCssTime("fast")).toBeNull();
  });

  it("falls back where no stylesheet is loaded", () => {
    expect(motionMs("--dur-exit", EXIT_MS)).toBe(EXIT_MS);
  });

  it("staggers at most six steps", () => {
    expect(staggerMs(0)).toBe(0);
    expect(staggerMs(2)).toBe(60);
    expect(staggerMs(40)).toBe(180);
    expect(staggerMs(-3)).toBe(0);
  });
});

describe("springEasingCss", () => {
  it("uses linear() where the engine has it, a cubic stand-in elsewhere", () => {
    expect(springEasingCss("smooth")).toMatch(/^cubic-bezier\(/);
    vi.stubGlobal("CSS", { supports: () => true });
    expect(springEasingCss("smooth")).toBe(springs.smooth.easing);
  });
});

describe("transitions", () => {
  it("enter on a spring and leave on the short exit", () => {
    const enter = resolve(appear(node, { delay: 40 }, { direction: "in" }), "in");
    expect(enter.duration).toBe(springs.smooth.duration);
    expect(enter.delay).toBe(40);
    expect(enter.easing).toBe(springs.smooth.at);
    const leave = resolve(appear(node, {}, { direction: "out" }), "out");
    expect(leave.duration).toBe(EXIT_MS);
  });

  it("pick the direction per run for transition:", () => {
    const both = splitOff(node, { from: 24 }, { direction: "both" });
    expect(typeof both).toBe("function");
    expect(resolve(both, "in").duration).toBe(springs.bouncy.duration);
    expect(resolve(both, "out").duration).toBeLessThan(springs.bouncy.duration);
  });

  it("animate only transform and opacity", () => {
    const configs = [
      resolve(appear(node, {}, { direction: "in" }), "in"),
      resolve(sheet(node, {}, { direction: "in" }), "in"),
      resolve(sheet(node, {}, { direction: "out" }), "out"),
      resolve(splitOff(node, {}, { direction: "in" }), "in"),
    ];
    for (const config of configs) {
      const css = config.css?.(0.5, 0.5) ?? "";
      const props = css
        .split(";")
        .map((d) => d.split(":")[0].trim())
        .filter(Boolean);
      expect(props.every((p) => ["opacity", "scale", "translate", "rotate", "transform"].includes(p))).toBe(true);
    }
  });

  it("only fade under Reduce Motion", () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    const config = resolve(sheet(node, {}, { direction: "in" }), "in");
    expect(config.css?.(0.5, 0.5)).toBe("opacity: 0.5");
  });
});

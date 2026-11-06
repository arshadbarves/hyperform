import { describe, expect, it } from "vitest";
import {
  FLING_LOOKAHEAD_MS,
  TURN_MS,
  WALK_MAX,
  flingAim,
  landSquash,
  moveDuration,
  planDrop,
  planFling,
  planHop,
  planMove,
  retargetMove,
  sampleMove,
  type MovePlan,
} from "./motion";

const composer = (x: number) => ({ x, y: 600, size: 46 });
const sidebar = { x: 120, y: 700, size: 40 };
const island = { x: 600, y: 40, size: 28 };
const window = { width: 1200, height: 800 };

function frames(plan: MovePlan, step = 4) {
  const out = [];
  for (let t = 0; t <= moveDuration(plan) + step; t += step) out.push(sampleMove(plan, t));
  return out;
}

describe("planMove", () => {
  it("walks along the same edge, hops across perches and further strolls", () => {
    expect(planMove(composer(300), composer(420), { sameEdge: true, facing: 1 })?.kind).toBe("walk");
    expect(planMove(composer(300), sidebar, { sameEdge: false, facing: 1 })?.kind).toBe("hop");
    expect(planMove(composer(0), composer(WALK_MAX + 40), { sameEdge: true, facing: 1 })?.kind).toBe("hop");
  });

  it("stays put for a tiny nudge, and faces where it goes, turning first", () => {
    expect(planMove(composer(300), composer(301), { sameEdge: true, facing: 1 })).toBeNull();
    const back = planMove(composer(300), composer(200), { sameEdge: true, facing: 1 });
    expect(back).toMatchObject({ facing: -1, windup: TURN_MS });
    expect(planMove(composer(300), composer(400), { sameEdge: true, facing: 1 })?.windup).toBe(0);
  });

  it("walks longer for longer strolls, in an even number of steps", () => {
    const short = planMove(composer(300), composer(360), { sameEdge: true, facing: 1 })!;
    const long = planMove(composer(300), composer(600), { sameEdge: true, facing: 1 })!;
    expect(long.travel).toBeGreaterThan(short.travel * 3);
    expect(long.steps % 2).toBe(0);
    expect(long.steps).toBeGreaterThan(short.steps);
  });
});

describe("a hop", () => {
  const hop = planMove(composer(300), sidebar, { sameEdge: false, facing: 1 })!;

  it("starts and ends on its perches", () => {
    expect(sampleMove(hop, 0)).toMatchObject({ x: 300, y: 600, size: 46 });
    expect(sampleMove(hop, moveDuration(hop) + 1)).toMatchObject({ x: 120, y: 700, size: 40, sx: 1, sy: 1, phase: "done" });
  });

  it("crouches first, arcs above both ends, and squashes on landing", () => {
    expect(sampleMove(hop, hop.windup * 0.9)).toMatchObject({ phase: "windup" });
    expect(sampleMove(hop, hop.windup * 0.9).sy).toBeLessThan(0.9);
    const air = frames(hop).filter((f) => f.phase === "air");
    expect(Math.min(...air.map((f) => f.y))).toBeLessThan(600 - 20);
    expect(air.some((f) => f.sy > 1.02)).toBe(true);
    const landing = sampleMove(hop, hop.windup + hop.travel + 1);
    expect(landing.phase).toBe("land");
    expect(landing.sy).toBeLessThan(1);
    expect(landing.sx).toBeGreaterThan(1);
  });

  it("keeps its head in the window when hopping up to the island", () => {
    const up = planMove(composer(300), island, { sameEdge: false, facing: 1, top: 28 })!;
    const lowest = Math.min(...frames(up).map((f) => f.y));
    expect(lowest).toBeGreaterThanOrEqual(27);
    expect(up.travel).toBeLessThanOrEqual(820);
  });

  it("hops on the spot to the height asked", () => {
    const joy = planHop(composer(300), 20, 1);
    const top = Math.min(...frames(joy, 1).map((f) => f.y));
    expect(top).toBeCloseTo(580, 0);
    expect(sampleMove(joy, moveDuration(joy) + 1)).toMatchObject({ x: 300, y: 600 });
  });
});

describe("a walk", () => {
  const stroll = planMove(composer(300), composer(500), { sameEdge: true, facing: 1 })!;

  it("bobs once per step and lifts one foot at a time", () => {
    const walking = frames(stroll, 2).filter((f) => f.phase === "walk");
    expect(walking.every((f) => f.lift[0] === 0 || f.lift[1] === 0)).toBe(true);
    expect(walking.some((f) => f.lift[0] > 1)).toBe(true);
    expect(walking.some((f) => f.lift[1] > 1)).toBe(true);
    expect(Math.min(...walking.map((f) => f.bob))).toBeLessThan(-0.9);
    const x = walking.map((f) => f.x);
    expect(x.every((v, i) => i === 0 || v >= x[i - 1])).toBe(true);
  });

  it("ends standing still on both feet", () => {
    const end = sampleMove(stroll, stroll.windup + stroll.travel - 0.01);
    expect(end.x).toBeCloseTo(500, 0);
    expect(Math.abs(end.bob)).toBeLessThan(0.05);
    expect(end.lift[0] + end.lift[1]).toBeLessThan(0.05);
  });
});

describe("letting go", () => {
  it("aims a fling along its release velocity", () => {
    expect(flingAim({ x: 100, y: 100 }, { x: 1, y: -0.5 })).toEqual({ x: 100 + FLING_LOOKAHEAD_MS, y: 100 - FLING_LOOKAHEAD_MS / 2 });
  });

  it("leaves with the speed it was flung at, and lands on the spot", () => {
    const v = { x: 1.2, y: -0.6 };
    const fling = planFling({ x: 400, y: 300, size: 46 }, v, composer(700), window);
    const a = sampleMove(fling, 0);
    const b = sampleMove(fling, 4);
    expect((b.x - a.x) / 4).toBeCloseTo(v.x, 1);
    expect((b.y - a.y) / 4).toBeCloseTo(v.y, 1);
    expect(sampleMove(fling, moveDuration(fling) + 1)).toMatchObject({ x: 700, y: 600 });
  });

  it("flips over when flung hard, and stays in the window", () => {
    const hard = planFling({ x: 400, y: 300, size: 46 }, { x: -3, y: -2 }, sidebar, window);
    expect(hard.spin).toBe(-360);
    expect(hard.control.x).toBeGreaterThanOrEqual(0);
    expect(hard.control.y).toBeGreaterThanOrEqual(40);
  });

  it("drops under gravity and bounces before it settles", () => {
    const drop = planDrop({ x: 300, y: 200, size: 46 }, composer(310), 1);
    expect(drop.kind).toBe("drop");
    expect(drop.bounces.length).toBeGreaterThan(0);
    const fall = frames(drop, 2).filter((f) => f.phase === "air").map((f) => f.y);
    expect(fall.every((y, i) => i === 0 || y >= fall[i - 1])).toBe(true);
    const bounce = frames(drop, 2).filter((f) => f.phase === "land");
    expect(Math.min(...bounce.map((f) => f.y))).toBeLessThan(600 - 5);
    expect(sampleMove(drop, moveDuration(drop) + 1)).toMatchObject({ x: 310, y: 600 });
  });

  it("hops onto a perch above where it was let go", () => {
    expect(planDrop({ x: 300, y: 700, size: 46 }, composer(300), 1).kind).toBe("hop");
  });
});

describe("retargetMove", () => {
  it("shifts a walk with its scrolling edge without restarting it", () => {
    const walk = planMove(composer(300), composer(420), { sameEdge: true, facing: 1 })!;
    const moved = retargetMove(walk, { x: 420, y: 560, size: 46 });
    const mid = moveDuration(walk) / 2;
    expect(sampleMove(moved, mid).y).toBeCloseTo(560);
    expect(sampleMove(moved, mid).x).toBeCloseTo(sampleMove(walk, mid).x);
    expect(moved.travel).toBe(walk.travel);
  });

  it("keeps a flight's start and bends its end onto the new spot", () => {
    const hop = planMove(sidebar, island, { sameEdge: false, facing: 1 })!;
    const moved = retargetMove(hop, { ...island, y: 20 });
    expect(sampleMove(moved, 0)).toMatchObject({ x: sidebar.x, y: sidebar.y });
    expect(sampleMove(moved, moveDuration(moved) + 1)).toMatchObject({ x: island.x, y: 20 });
  });
});

describe("landSquash", () => {
  it("squashes harder for a harder landing and settles to round", () => {
    expect(landSquash(0, 1).sy).toBeLessThan(landSquash(0, 0.3).sy);
    expect(landSquash(400, 1).sy).toBeCloseTo(1, 2);
  });
});

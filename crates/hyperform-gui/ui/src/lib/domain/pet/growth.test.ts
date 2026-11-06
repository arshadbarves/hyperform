import { describe, expect, it } from "vitest";
import {
  applyGrowth,
  COUNTED_MAX,
  emptyGrowth,
  levelForXp,
  levelProgress,
  localDay,
  parseGrowth,
  wear,
  XP,
  xpForLevel,
  type GrowthEvent,
} from "./growth";

const turn = (turnId: string, over: Partial<Extract<GrowthEvent, { kind: "turn" }>> = {}): GrowthEvent => ({
  kind: "turn",
  turnId,
  completed: true,
  verified: false,
  day: "2026-09-29",
  ...over,
});

describe("pet growth", () => {
  it("follows a level curve that asks a little more each level", () => {
    expect([1, 2, 3, 4, 5, 10].map(xpForLevel)).toEqual([0, 50, 150, 300, 500, 2250]);
    expect(levelForXp(0)).toBe(1);
    expect(levelForXp(49)).toBe(1);
    expect(levelForXp(50)).toBe(2);
    expect(levelForXp(2249)).toBe(9);
    expect(levelForXp(2250)).toBe(10);
    expect(levelProgress(100)).toEqual({ level: 2, into: 50, span: 100, fraction: 0.5 });
  });

  it("rewards completed turns, more when their checks pass, plus the first of the day", () => {
    const first = applyGrowth(emptyGrowth(), turn("t1", { verified: true }));
    expect(first.gained).toBe(XP.turn + XP.verified + XP.firstOfDay);
    expect(first.state).toMatchObject({ turns: 1, verified: 1, streak: 1, lastDay: "2026-09-29" });
    const second = applyGrowth(first.state, turn("t2"));
    expect(second.gained).toBe(XP.turn);
  });

  it("counts each turn once and adds nothing for failures or cancels", () => {
    const once = applyGrowth(emptyGrowth(), turn("t1")).state;
    expect(applyGrowth(once, turn("t1")).gained).toBe(0);
    const cancelled = applyGrowth(once, turn("t2", { completed: false }));
    expect(cancelled.gained).toBe(0);
    expect(cancelled.state).toBe(once);
  });

  it("never loses XP, and keeps a streak of consecutive days", () => {
    let state = applyGrowth(emptyGrowth(), turn("a", { day: "2026-09-28" })).state;
    state = applyGrowth(state, turn("b", { day: "2026-09-29" })).state;
    expect(state.streak).toBe(2);
    const xp = state.xp;
    state = applyGrowth(state, turn("c", { day: "2026-10-05" })).state;
    expect(state.streak).toBe(1);
    expect(state.xp).toBeGreaterThan(xp);
  });

  it("rewards applying a helper's changes and reports what a level-up unlocks", () => {
    let state = emptyGrowth();
    state = { ...state, xp: 40, lastDay: "2026-09-29", streak: 1 };
    const result = applyGrowth(state, { kind: "applied", agentId: "A1", day: "2026-09-29" });
    expect(result.gained).toBe(XP.applied);
    expect(result).toMatchObject({ levelFrom: 1, levelTo: 2, unlocked: { accessories: ["sprout"], tricks: [] } });
    expect(applyGrowth(result.state, { kind: "applied", agentId: "A1", day: "2026-09-29" }).gained).toBe(0);
  });

  it("remembers only the latest counted events", () => {
    let state = emptyGrowth();
    for (let i = 0; i < COUNTED_MAX + 5; i++) state = applyGrowth(state, turn(`t${i}`)).state;
    expect(state.counted).toHaveLength(COUNTED_MAX);
    expect(state.counted[0]).toBe("turn:t5");
  });

  it("reads saved growth defensively", () => {
    expect(parseGrowth(null)).toEqual(emptyGrowth());
    expect(parseGrowth({ xp: -5, turns: 3.7, lastDay: "yesterday", wearing: "hat", counted: ["a", 1] })).toMatchObject({
      xp: 0,
      turns: 3,
      lastDay: null,
      streak: 0,
      wearing: null,
      counted: ["a"],
    });
    expect(parseGrowth({ xp: 60, lastDay: "2026-09-01", streak: 0, wearing: "sprout" })).toMatchObject({ streak: 1, wearing: "sprout" });
  });

  it("wears only what is unlocked", () => {
    const young = { ...emptyGrowth(), xp: 60 };
    expect(wear(young, "sprout").wearing).toBe("sprout");
    expect(wear(young, "star").wearing).toBeNull();
    expect(wear({ ...young, wearing: "sprout" }, null).wearing).toBeNull();
  });

  it("names local days", () => {
    expect(localDay(new Date(2026, 0, 5, 23, 30))).toBe("2026-01-05");
  });
});

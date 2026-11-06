import { describe, expect, it } from "vitest";
import { turnRecord } from "../testFixtures";
import { emptyView } from "../sessionView/types";
import { latestTurnTone, toneMood } from "./decisionMood";

describe("decision mood", () => {
  it("reads the tone of the latest finished turn only", () => {
    const view = { ...emptyView("S1"), turns: [turnRecord({ turnId: "t1" }), turnRecord({ turnId: "t2" })] };
    expect(latestTurnTone(view)).toBeNull();
    expect(latestTurnTone({ ...view, turnTones: { t1: "blocked" } })).toBeNull();
    expect(latestTurnTone({ ...view, turnTones: { t2: "done_well" } })).toBe("done_well");
    expect(latestTurnTone(null)).toBeNull();
    expect(latestTurnTone(emptyView("S2"))).toBeNull();
  });

  it("colors the outcome without contradicting it", () => {
    expect(toneMood("done_well", "ok")).toBe("proud");
    expect(toneMood("smooth", "quiet")).toBe("content");
    expect(toneMood("struggling", "ok")).toBe("relieved");
    expect(toneMood("blocked", "attention")).toBe("worried");
    expect(toneMood("blocked", "ok")).toBeNull();
    for (const tone of ["done_well", "smooth", "struggling", "blocked"] as const) {
      expect(toneMood(tone, "danger")).toBeNull();
    }
    expect(toneMood(null, "ok")).toBeNull();
  });
});

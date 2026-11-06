import { describe, expect, it } from "vitest";
import { EASE } from "../domain/pet/keyframes";
import { moving, restart, retarget, still, valueAt } from "./tween";

describe("tweens", () => {
  const at = (x: number) => ({ x });

  it("ease from where they had got to and hold the target", () => {
    let tween = retarget(still(at(0)), at(10), 0, 100, EASE.linear);
    expect(valueAt(tween, 50).x).toBeCloseTo(5);
    expect(moving(tween, 50)).toBe(true);
    tween = retarget(tween, at(0), 50, 100, EASE.linear);
    expect(valueAt(tween, 50).x).toBeCloseTo(5);
    expect(valueAt(tween, 150).x).toBe(0);
    expect(moving(tween, 150)).toBe(false);
  });

  it("keep their tween for the same target and jump with no time", () => {
    const tween = retarget(still(at(0)), at(4), 0, 100);
    expect(retarget(tween, at(4), 60, 100)).toBe(tween);
    expect(retarget(tween, at(9), 60, 0).to.x).toBe(9);
    expect(moving(restart(at(0), at(0), 0, 100), 10)).toBe(false);
  });
});

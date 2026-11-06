import { describe, expect, it } from "vitest";
import type { BodyMotion } from "./emotions";
import { FACE_YAW, REST_POSE, animates, bodyPose, faceYaw, threeAxes, type BodyPose } from "./rig3d";
import { ROUTINE_MS, type PetRoutine } from "./routines";

const deg = (d: number) => (d * Math.PI) / 180;

/** Each mood's body motion and its loop's period in pet-moves.css (null: a still pose). */
const PERIOD: Record<BodyMotion, number | null> = {
  breathe: null,
  bob: 1100,
  sway: 3000,
  nod: 2400,
  lean: null,
  type: 320,
  tilt: null,
  sweep: 900,
  bounce: 1500,
  puff: 2800,
  sigh: 3200,
  slump: null,
  tremble: 1800,
  squish: 1300,
  wiggle: 800,
  wave: 1200,
  sink: null,
  yawn: 1900,
  doze: 4400,
  sleep: null,
};
const BODIES = Object.keys(PERIOD) as BodyMotion[];
const ROUTINES = Object.keys(ROUTINE_MS) as PetRoutine[];
const TIMES = Array.from({ length: 61 }, (_, i) => i * 50);
/** Motions with a still part under their loop. */
const HELD = new Set<BodyMotion>(["lean", "tilt", "puff", "slump", "tremble", "sink", "sleep"]);

const numbers = (p: BodyPose) => [p.x, p.y, p.z, p.rotX, p.rotY, p.rotZ, p.sx, p.sy, p.sz, p.aura, ...p.lift];
const key = (p: BodyPose) => numbers(p).map((n) => n.toFixed(4)).join(" ");

function expectPose(actual: BodyPose, expected: BodyPose, label: string) {
  const want = numbers(expected);
  numbers(actual).forEach((n, i) => expect(n, `${label} [${i}]`).toBeCloseTo(want[i], 6));
}

describe("bodyPose", () => {
  it("gives every body motion and routine a finite pose at any time", () => {
    for (const body of BODIES) {
      for (const routine of [null, ...ROUTINES]) {
        for (const t of [-10, ...TIMES, 12_345]) {
          const pose = bodyPose(body, routine, t);
          expect(numbers(pose).every(Number.isFinite), `${body} ${routine} ${t}`).toBe(true);
          expect(Math.min(pose.sx, pose.sy, pose.sz)).toBeGreaterThan(0.5);
        }
      }
    }
  });

  it("starts each loop at rest and comes back to it every period", () => {
    for (const body of BODIES) {
      const period = PERIOD[body];
      if (period === null) continue;
      const start = bodyPose(body, null, 0);
      if (!HELD.has(body)) expectPose(start, REST_POSE, body);
      expectPose(bodyPose(body, null, period), start, `${body} one period`);
      expectPose(bodyPose(body, null, period * 3), start, `${body} three periods`);
      const moved = TIMES.filter((t) => t < period).some((t) => key(bodyPose(body, null, t)) !== key(start));
      expect(moved, body).toBe(true);
    }
  });

  it("holds the still poses, and leaves breathing to the CSS wrapper", () => {
    for (const body of BODIES) {
      if (PERIOD[body] !== null) continue;
      const first = bodyPose(body, null, 0);
      for (const t of TIMES) expectPose(bodyPose(body, null, t), first, `${body} ${t}`);
    }
    expectPose(bodyPose("breathe", null, 777), REST_POSE, "breathe");
    expect(bodyPose("lean", null, 0)).toMatchObject({ x: 0.6, rotZ: deg(7) });
    expect(bodyPose("tilt", null, 0).rotZ).toBeCloseTo(deg(-9));
    expect(bodyPose("sink", null, 0).y).toBe(5);
    expect(bodyPose("sleep", null, 0)).toMatchObject({ y: 1.2, sx: 1.06, sy: 0.9, sz: 1.06 });
    expect(bodyPose("puff", null, 0)).toMatchObject({ sx: 1.05, sy: 1.03 });
  });

  it("rolls the 2D rotations, pitches the nod forward and squashes depth with width", () => {
    expect(bodyPose("sway", null, 750).rotZ).toBeCloseTo(deg(-4));
    expect(bodyPose("wiggle", null, 600).rotZ).toBeCloseTo(deg(6));
    expect(bodyPose("wave", null, 300).rotZ).toBeCloseTo(deg(-12));
    const doze = bodyPose("doze", null, 4400 * 0.55);
    expect(doze.rotZ).toBeCloseTo(deg(7));
    expect(doze.y).toBeCloseTo(0.8);
    expect(doze.rotX).toBe(0);
    const nod = bodyPose("nod", null, 1800);
    expect(nod.rotX).toBeCloseTo(deg(5));
    expect(nod.y).toBeCloseTo(0.5);
    expect(nod.rotZ).toBe(0);
    const bounce = bodyPose("bounce", null, 450);
    expect(bounce.y).toBeCloseTo(-3.2);
    expect(bounce.sy).toBeCloseTo(1.06);
    expect(bounce.sz).toBe(bounce.sx);
  });

  it("waves twice and yawns once, then rests", () => {
    expect(bodyPose("wave", null, 1500).rotZ).toBeCloseTo(deg(-12));
    expectPose(bodyPose("wave", null, 2400), REST_POSE, "wave");
    expect(bodyPose("yawn", null, 1000).sy).toBeCloseTo(1.08);
    expectPose(bodyPose("yawn", null, 5000), REST_POSE, "yawn");
  });
});

describe("routines", () => {
  const at = (routine: PetRoutine, t: number) => bodyPose("breathe", routine, t);

  it("sits and lies down as still squashes on top of the mood", () => {
    expect(at("sit", 5000)).toMatchObject({ y: 0.8, sx: 1.05, sy: 0.94 });
    expect(at("lieDown", 100)).toMatchObject({ y: 2.2, sx: 1.14, sy: 0.8, sz: 1.14 });
    const swaying = bodyPose("sway", "sit", 750);
    expect(swaying.rotZ).toBeCloseTo(deg(-4));
    expect(swaying.sy).toBeCloseTo(0.94);
  });

  it("spins a full turn about the vertical axis and ends facing front", () => {
    expect(at("spin", 0).rotY).toBe(0);
    expect(Math.max(...TIMES.filter((t) => t < 1000).map((t) => at("spin", t).rotY))).toBeGreaterThan(1.8 * Math.PI);
    expect(at("spin", 500).rotZ).toBe(0);
    expectPose(at("spin", 1000), REST_POSE, "spin done");
  });

  it("looks left, then right, by turning its body", () => {
    expect(at("lookAround", 2600 * 0.3).rotY).toBeCloseTo(-deg(25));
    expect(at("lookAround", 2600 * 0.7).rotY).toBeCloseTo(deg(25));
    expectPose(at("lookAround", 2600), REST_POSE, "lookAround done");
  });

  it("taps its right foot four times", () => {
    expect(at("tap", 225).lift[0]).toBe(0);
    expect(at("tap", 225).lift[1]).toBeCloseTo(1.1);
    expect(at("tap", 450 * 3 + 225).lift[1]).toBeCloseTo(1.1);
    expectPose(at("tap", ROUTINE_MS.tap), REST_POSE, "tap done");
    expect(bodyPose("sway", "tap", 0, { routineMs: 225 }).lift[1]).toBeCloseTo(1.1);
  });

  it("stretches tall, and sparkles by making its aura glow", () => {
    expect(at("stretch", 560).sy).toBeCloseTo(1.14);
    expect(at("sparkle", 700).aura).toBeCloseTo(8 / 3);
    expect(at("sparkle", ROUTINE_MS.sparkle).aura).toBe(1);
  });

  it("leaves the hop to the motion engine, and the wave and yawn to their moods", () => {
    for (const routine of ["hop", "wave", "yawn"] as const) {
      for (const t of TIMES) expectPose(at(routine, t), REST_POSE, `${routine} ${t}`);
    }
  });
});

describe("the aura", () => {
  it("pulses for the big feelings, and the sparkle trick's glow wins while it plays", () => {
    for (const mood of ["excited", "proud", "love"] as const) {
      expect(bodyPose("bounce", null, 600, { mood }).aura).toBeCloseTo(8 / 3);
    }
    expect(bodyPose("bounce", null, 600, { mood: "happy" }).aura).toBe(1);
    expect(bodyPose("bounce", "sparkle", 600, { mood: "excited", routineMs: 0 }).aura).toBeCloseTo(2 / 3);
  });
});

describe("animates", () => {
  it("needs frames exactly when the pose changes over time", () => {
    for (const body of BODIES) {
      for (const routine of [null, ...ROUTINES]) {
        const changes = new Set(TIMES.map((t) => key(bodyPose(body, routine, t)))).size > 1;
        expect(animates(body, routine), `${body} ${routine}`).toBe(changes);
      }
    }
  });

  it("stops counting a one-shot once it has ended", () => {
    expect(animates("wave", null, 1000)).toBe(true);
    expect(animates("wave", null, 2400)).toBe(false);
    expect(animates("yawn", null, 1899)).toBe(true);
    expect(animates("yawn", null, 1900)).toBe(false);
    expect(animates("bob", null, 1e7)).toBe(true);
    expect(animates("breathe", "spin", 500)).toBe(true);
    expect(animates("breathe", "spin", 0, { routineMs: 1000 })).toBe(false);
    expect(animates("breathe", "sit")).toBe(false);
  });

  it("counts the big feelings' aura pulse", () => {
    expect(animates("breathe", null, 0, { mood: "love" })).toBe(true);
    expect(animates("breathe", null, 0, { mood: "idle" })).toBe(false);
  });
});

describe("facing", () => {
  it("turns about thirty degrees each way, passing the front mid-turn", () => {
    expect(FACE_YAW).toBeCloseTo(deg(30));
    expect(faceYaw(1)).toBeCloseTo(FACE_YAW);
    expect(faceYaw(-1)).toBeCloseTo(-FACE_YAW);
    expect(faceYaw(0)).toBe(0);
    expect(faceYaw(0.5)).toBeCloseTo(FACE_YAW / 2);
    expect(faceYaw(3)).toBeCloseTo(FACE_YAW);
    expect(faceYaw(Number.NaN)).toBe(0);
  });

  it("hands Three.js its axes: y up, and a roll the other way", () => {
    const pose = bodyPose("nod", null, 1800);
    const three = threeAxes({ ...pose, rotZ: 0.2, rotY: 0.3 });
    expect(three.position[1]).toBeCloseTo(-pose.y);
    expect(three.rotation).toEqual([pose.rotX, 0.3, -0.2]);
    expect(three.scale).toEqual([pose.sx, pose.sy, pose.sz]);
  });
});

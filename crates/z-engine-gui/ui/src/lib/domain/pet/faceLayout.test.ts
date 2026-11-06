import { describe, expect, it } from "vitest";
import type { Blush, BrowShape, EyeShape } from "./emotions";
import {
  BLINK,
  POP_MS,
  TEAR_LATE_MS,
  YAWN_MS,
  blushLayout,
  browLayout,
  drawnEyeMotion,
  eyeLayout,
  faceAnimates,
  mouthScale,
  popIn,
  scanOffset,
  showsGlint,
  sweatDrip,
  tearDrip,
  type PartTransform,
} from "./faceLayout";
import { isDrawnEye } from "./faceShapes";

const deg = (d: number) => (d * Math.PI) / 180;
const finite = (t: PartTransform) => Object.values(t).every(Number.isFinite);

/** Each eye shape's [sx, sy, dy] for the left and right eye, from pet-face.css; null when drawn. */
const EYES: Record<EyeShape, [number, number, number][] | null> = {
  open: [[1, 1, 0], [1, 1, 0]],
  wide: [[1.2, 1.2, 0], [1.2, 1.2, 0]],
  narrow: [[1, 0.6, 0], [1, 0.6, 0]],
  half: [[1, 0.4, 0.6], [1, 0.4, 0.6]],
  sad: [[0.95, 0.8, 0.4], [0.95, 0.8, 0.4]],
  puppy: [[1.3, 1.25, 0], [1.3, 1.25, 0]],
  uneven: [[1.16, 1.16, 0], [1, 0.62, 0.3]],
  closed: null,
  happy: null,
  star: null,
  heart: null,
  spiral: null,
};

/** Each brow shape's opacity and [dy, rotation in degrees] for the left and right brow. */
const BROWS: Record<BrowShape, [number, [number, number], [number, number]]> = {
  none: [0, [0, 0], [0, 0]],
  raised: [0.85, [-1.1, 0], [-1.1, 0]],
  relaxed: [0.55, [-0.4, 0], [-0.4, 0]],
  worried: [0.85, [-0.5, -15], [-0.5, 15]],
  sad: [0.85, [0.2, -22], [0.2, 22]],
  firm: [0.85, [0.6, 14], [0.6, -14]],
  uneven: [0.85, [-1.3, -8], [0.3, 6]],
};

const BLUSH: Record<Blush, number> = { none: 0.22, soft: 0.55, strong: 0.85 };

describe("eyeLayout", () => {
  it("poses both ellipse eyes for every open shape, and hands the drawn ones over", () => {
    for (const [eyes, expected] of Object.entries(EYES) as [EyeShape, [number, number, number][] | null][]) {
      const layout = eyeLayout(eyes);
      expect(layout.drawn, eyes).toBe(isDrawnEye(eyes));
      expect(finite(layout.left) && finite(layout.right), eyes).toBe(true);
      if (!expected) continue;
      for (const [side, [sx, sy, dy]] of [
        [layout.left, expected[0]],
        [layout.right, expected[1]],
      ] as const) {
        expect(side, eyes).toMatchObject({ sx, sy, dy, dx: 0, rot: 0 });
      }
    }
  });

  it("gives a seed bigger open eyes, and blinks by squeezing them flat", () => {
    expect(eyeLayout("open", "seed").left.sx).toBe(1.12);
    expect(eyeLayout("wide", "seed").left.sx).toBe(1.2);
    expect(BLINK).toMatchObject({ sx: 1, sy: 0.1, dy: 0 });
    expect(showsGlint("puppy", false)).toBe(true);
    expect(showsGlint("puppy", true)).toBe(false);
    expect(showsGlint("open", false)).toBe(false);
  });
});

describe("browLayout", () => {
  it("poses every brow shape, hidden unless the mood needs them", () => {
    for (const [shape, [opacity, [ly, lr], [ry, rr]]] of Object.entries(BROWS) as [BrowShape, (typeof BROWS)["none"]][]) {
      const brows = browLayout(shape);
      expect(brows.opacity, shape).toBe(opacity);
      expect(brows.left.dy, shape).toBeCloseTo(ly);
      expect(brows.left.rot, shape).toBeCloseTo(deg(lr));
      expect(brows.right.dy, shape).toBeCloseTo(ry);
      expect(brows.right.rot, shape).toBeCloseTo(deg(rr));
    }
  });

  it("mirrors worried and sad brows, so they slope toward the middle", () => {
    expect(browLayout("worried").left.rot).toBeCloseTo(-browLayout("worried").right.rot);
    expect(browLayout("sad").left.rot).toBeLessThan(0);
    expect(browLayout("firm").left.rot).toBeGreaterThan(0);
  });
});

describe("blushLayout", () => {
  it("blushes more for each blush, and a strong blush swells the cheeks", () => {
    for (const [blush, opacity] of Object.entries(BLUSH) as [Blush, number][]) {
      expect(blushLayout(blush).opacity, blush).toBe(opacity);
    }
    expect(blushLayout("strong")).toMatchObject({ sx: 1.18, sy: 1.1 });
    expect(blushLayout("soft")).toMatchObject({ sx: 1, sy: 1 });
  });

  it("keeps a little colour in a grown pet's cheeks", () => {
    expect(blushLayout("none", "bloom").opacity).toBe(0.34);
    expect(blushLayout("none", "star").opacity).toBe(0.34);
    expect(blushLayout("none", "seed").opacity).toBe(0.22);
    expect(blushLayout("soft", "star").opacity).toBe(0.55);
  });
});

describe("the face's own motion", () => {
  it("chatters while talking and gapes once for a yawn, which then stays small", () => {
    expect(mouthScale("talk", 0).sy).toBeCloseTo(0.55);
    expect(mouthScale("talk", 170).sy).toBeCloseTo(1.15);
    expect(mouthScale("talk", 340).sy).toBeCloseTo(0.55);
    for (const [t, sx, sy] of [
      [0, 0.5, 0.3],
      [YAWN_MS * 0.55, 1, 1.1],
      [YAWN_MS + 500, 0.5, 0.3],
    ]) {
      expect(mouthScale("yawn", t).sx).toBeCloseTo(sx);
      expect(mouthScale("yawn", t).sy).toBeCloseTo(sy);
    }
    expect(mouthScale("smile", 170)).toMatchObject({ sx: 1, sy: 1 });
    expect(mouthScale("none", 0)).toMatchObject({ sx: 1, sy: 1 });
  });

  it("scales the talking and yawning mouths from near their top", () => {
    expect(mouthScale("talk", 0).pivot.y).toBeCloseTo(23 - 0.9 * 0.6);
    expect(mouthScale("yawn", 0).pivot).toEqual({ x: 16, y: 23.3 - 1.8 * 0.6 });
  });

  it("pops new parts in, then leaves them whole", () => {
    expect(popIn("eyes", 0)).toMatchObject({ scale: 0.4, opacity: 0 });
    expect(popIn("mouth", POP_MS.mouth / 2).scale).toBeGreaterThan(0.7);
    expect(popIn("mouth", POP_MS.mouth + 1).scale).toBeCloseTo(1);
    expect(popIn("eyes", POP_MS.eyes + 1).opacity).toBe(1);
  });

  it("pulses star and heart eyes once they are in, and spins spiral eyes", () => {
    expect(drawnEyeMotion("star", 100).scale).toBe(1);
    expect(drawnEyeMotion("heart", POP_MS.eyes + 450).scale).toBeCloseTo(1.16);
    expect(drawnEyeMotion("spiral", 550).rot).toBeCloseTo(Math.PI);
    expect(drawnEyeMotion("open", 550)).toEqual({ scale: 1, rot: 0 });
  });

  it("drips sweat and tears, the right tear after the left", () => {
    expect(sweatDrip(0)).toMatchObject({ opacity: 0, dy: -0.6 });
    expect(sweatDrip(2400 * 0.4).opacity).toBeCloseTo(0.9);
    expect(sweatDrip(2400 * 0.99).dy).toBeGreaterThan(2);
    expect(tearDrip("right", TEAR_LATE_MS)).toEqual(tearDrip("left", 0));
    expect(tearDrip("right", 100).opacity).toBe(0);
  });

  it("scans side to side, faster while searching", () => {
    expect(scanOffset(400).x).toBeCloseTo(-1.8);
    expect(scanOffset(1200).x).toBeCloseTo(1.8);
    expect(scanOffset(225, true).x).toBeCloseTo(-1.8);
    expect(scanOffset(0)).toEqual({ x: 0, y: 0 });
  });

  it("needs frames while a loop plays or a part is still arriving", () => {
    const still = { eyes: "open", brows: "none", mouth: "smile", blush: "none", sweat: false, tears: false } as const;
    expect(faceAnimates(still, 10_000)).toBe(false);
    expect(faceAnimates(still, 100)).toBe(true);
    expect(faceAnimates({ ...still, mouth: "talk" }, 10_000)).toBe(true);
    expect(faceAnimates({ ...still, eyes: "spiral" }, 10_000)).toBe(true);
    expect(faceAnimates({ ...still, sweat: true }, 10_000)).toBe(true);
    expect(faceAnimates({ ...still, mouth: "yawn" }, YAWN_MS - 10)).toBe(true);
    expect(faceAnimates({ ...still, mouth: "yawn" }, YAWN_MS + 10)).toBe(false);
  });
});

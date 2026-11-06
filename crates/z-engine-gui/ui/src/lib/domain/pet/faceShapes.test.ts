import { describe, expect, it } from "vitest";
import { canBlink, type EyeShape, type MouthShape } from "./emotions";
import { BROWS, CHEEKS, EYES, EYE_GLYPHS, EYE_LINES, GLINTS, MOUTHS, SWEAT, TEARS, isDrawnEye } from "./faceShapes";

const EYE_SHAPES = Object.keys({
  open: 1,
  wide: 1,
  narrow: 1,
  half: 1,
  closed: 1,
  sad: 1,
  puppy: 1,
  uneven: 1,
  happy: 1,
  star: 1,
  heart: 1,
  spiral: 1,
} satisfies Record<EyeShape, 1>) as EyeShape[];

const MOUTH_SHAPES = Object.keys({
  none: 1,
  smile: 1,
  grin: 1,
  laugh: 1,
  o: 1,
  flat: 1,
  frown: 1,
  wobble: 1,
  yawn: 1,
  tongue: 1,
  cat: 1,
  smug: 1,
  talk: 1,
} satisfies Record<MouthShape, 1>) as MouthShape[];

const isPath = (d: string) => /^M[-\d.]/.test(d) && d.length > 4;
const inBox = (...values: number[]) => values.every((v) => Number.isFinite(v) && v >= -3 && v <= 32);

describe("face shapes", () => {
  it("draws every mouth but none, each part a path or an ellipse in the 32-unit box", () => {
    for (const mouth of MOUTH_SHAPES) {
      if (mouth === "none") continue;
      const parts = MOUTHS[mouth];
      expect(parts.length, mouth).toBeGreaterThan(0);
      for (const part of parts) {
        if (part.kind === "path") expect(isPath(part.d), mouth).toBe(true);
        else expect(inBox(part.cx, part.cy, part.rx, part.ry) && part.rx > 0 && part.ry > 0, mouth).toBe(true);
      }
    }
    expect(Object.keys(MOUTHS)).not.toContain("none");
  });

  it("has a drawing for every drawn eye, and ellipses for the rest", () => {
    for (const eyes of EYE_SHAPES) {
      if (!isDrawnEye(eyes)) continue;
      const d = eyes in EYE_LINES ? EYE_LINES[eyes as keyof typeof EYE_LINES] : EYE_GLYPHS[eyes as keyof typeof EYE_GLYPHS];
      expect(isPath(d), eyes).toBe(true);
    }
    expect(EYE_SHAPES.filter(isDrawnEye).sort()).toEqual(["closed", "happy", "heart", "spiral", "star"]);
    for (const eyes of EYE_SHAPES) if (canBlink(eyes)) expect(isDrawnEye(eyes), eyes).toBe(false);
  });

  it("mirrors the two sides of the face about its middle", () => {
    for (const pair of [CHEEKS, EYES, BROWS]) {
      expect(pair.left.cx + pair.right.cx).toBeCloseTo(32);
      expect(pair.left.cy).toBe(pair.right.cy);
    }
    expect(inBox(EYES.left.rx, EYES.left.ry, GLINTS.left.cx, GLINTS.left.r)).toBe(true);
    for (const d of [BROWS.left.d, BROWS.right.d, SWEAT, TEARS.left, TEARS.right]) expect(isPath(d)).toBe(true);
  });
});

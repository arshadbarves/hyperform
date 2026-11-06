import { Color } from "three";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MouthShape } from "../domain/pet/emotions";
import { IDENTITY, browLayout } from "../domain/pet/faceLayout";
import { paintFace, paintKey, type FaceInk, type FacePaint } from "./faceTexture";

/** A 2D context that records what was drawn. */
function recorder() {
  const calls: string[] = [];
  const gradient = { addColorStop: () => {} };
  const ctx = new Proxy({} as Record<string, unknown>, {
    get: (target, key: string) =>
      key in target ? target[key] : () => (calls.push(key), key === "createRadialGradient" ? gradient : undefined),
    set: (target, key: string, value) => ((target[key] = value), true),
  });
  return { ctx: ctx as unknown as CanvasRenderingContext2D, calls };
}

const ink: FaceInk = {
  eye: new Color("#1e2230"),
  blush: new Color("#ff9aa8"),
  mouth: new Color("#5a2d3a"),
  gold: new Color("#ffd35a"),
  heart: new Color("#ff5f7a"),
  sweat: new Color("#9fd8ff"),
};
const glyph = { scale: 1, opacity: 1, rot: 0 };

function face(over: Partial<FacePaint> = {}): FacePaint {
  return {
    cheeks: { opacity: 0, sx: 1, sy: 1 },
    brows: browLayout("none"),
    eyes: null,
    mouth: null,
    tears: null,
    ...over,
  };
}

beforeEach(() => vi.stubGlobal("Path2D", class {}));
afterEach(() => vi.unstubAllGlobals());

describe("the face texture", () => {
  it("draws nothing but a cleared square for a bare face", () => {
    const { ctx, calls } = recorder();
    paintFace(ctx, 256, face(), ink);
    expect(calls).toEqual(["setTransform", "clearRect"]);
  });

  it("paints blush, brows, drawn eyes, mouth and tears", () => {
    const { ctx, calls } = recorder();
    paintFace(
      ctx,
      256,
      face({
        cheeks: { opacity: 0.8, sx: 1, sy: 1 },
        brows: browLayout("raised"),
        eyes: { shape: "star", left: glyph, right: glyph },
        mouth: { shape: "grin", pop: 1, opacity: 1, ...IDENTITY, pivot: { x: 16, y: 23 } },
        tears: { left: { opacity: 1, dy: 0 }, right: { opacity: 1, dy: 1 } },
      }),
      ink,
    );
    expect(calls.filter((c) => c === "createRadialGradient")).toHaveLength(2);
    expect(calls.filter((c) => c === "stroke").length).toBeGreaterThanOrEqual(2);
    expect(calls.filter((c) => c === "fill").length).toBeGreaterThanOrEqual(5);
  });

  it.each<Exclude<MouthShape, "none">>(["smile", "grin", "laugh", "o", "flat", "frown", "wobble", "yawn", "tongue", "cat", "smug", "talk"])(
    "draws the %s mouth",
    (shape) => {
      const { ctx, calls } = recorder();
      paintFace(ctx, 128, face({ mouth: { shape, pop: 1, opacity: 1, ...IDENTITY, pivot: { x: 16, y: 23 } } }), ink);
      expect(calls.some((c) => c === "fill" || c === "stroke")).toBe(true);
    },
  );

  it.each(["happy", "closed", "heart", "spiral"] as const)("draws %s eyes", (shape) => {
    const { ctx, calls } = recorder();
    paintFace(ctx, 128, face({ eyes: { shape, left: glyph, right: glyph } }), ink);
    expect(calls.some((c) => c === "fill" || c === "stroke")).toBe(true);
  });

  it("keys a face by what it shows, ignoring rounding noise", () => {
    expect(paintKey(face({ cheeks: { opacity: 0.5, sx: 1, sy: 1 } }))).toBe(paintKey(face({ cheeks: { opacity: 0.50001, sx: 1, sy: 1 } })));
    expect(paintKey(face())).not.toBe(paintKey(face({ cheeks: { opacity: 0.5, sx: 1, sy: 1 } })));
  });
});

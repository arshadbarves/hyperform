import type { EyeShape, MouthShape } from "./emotions";

/**
 * The shapes of the pet's face in its 32×32 space (y down): cheeks, brows,
 * eyes, every mouth, a sweat drop and tears. The SVG face (PetFace) and
 * the 3D face texture both draw from these; the path strings also work
 * as `new Path2D(d)`. How each shape is posed is in faceLayout.ts.
 */

export type Side = "left" | "right";

export interface Ellipse {
  cx: number;
  cy: number;
  rx: number;
  ry: number;
}

export interface Circle {
  cx: number;
  cy: number;
  r: number;
}

/** A stroked line and the centre it turns about. */
export interface Line {
  d: string;
  cx: number;
  cy: number;
}

export const CHEEKS: Record<Side, Ellipse> = {
  left: { cx: 9.3, cy: 21.7, rx: 1.9, ry: 1.1 },
  right: { cx: 22.7, cy: 21.7, rx: 1.9, ry: 1.1 },
};

export const BROWS: Record<Side, Line> = {
  left: { d: "M10.9 14.6h3", cx: 12.4, cy: 14.6 },
  right: { d: "M18.1 14.6h3", cx: 19.6, cy: 14.6 },
};

/** The open eyes; drawn eyes are centred on the same points. */
export const EYES: Record<Side, Ellipse> = {
  left: { cx: 12.4, cy: 18.4, rx: 1.35, ry: 2 },
  right: { cx: 19.6, cy: 18.4, rx: 1.35, ry: 2 },
};

/** The glints in puppy eyes. */
export const GLINTS: Record<Side, Circle> = {
  left: { cx: 12.9, cy: 17.4, r: 0.55 },
  right: { cx: 20.1, cy: 17.4, r: 0.55 },
};

/** Drawn eyes that are one stroked path across both eyes, in place. */
export const EYE_LINES = {
  happy: "M11.1 19.1q1.3-1.9 2.6 0M18.3 19.1q1.3-1.9 2.6 0",
  closed: "M11.1 18.3q1.3 1.3 2.6 0M18.3 18.3q1.3 1.3 2.6 0",
} as const;

/** Drawn eyes that are a glyph around (0, 0), placed at each eye's centre. Star and heart fill; the spiral strokes. */
export const EYE_GLYPHS = {
  star: "M0-2.3l.7 1.6 1.6.7-1.6.7-.7 1.6-.7-1.6-1.6-.7 1.6-.7z",
  heart: "M0 1.8s-2-1.3-2-2.7c0-.8.6-1.3 1.2-1.3.4 0 .7.2.8.5.1-.3.4-.5.8-.5.6 0 1.2.5 1.2 1.3 0 1.4-2 2.7-2 2.7z",
  spiral: "M0 0a.4.4 0 1 1 .8 0a.8.8 0 1 1-1.6 0a1.2 1.2 0 1 1 2.4 0a1.6 1.6 0 1 1-3.2 0",
} as const;

export type DrawnEye = keyof typeof EYE_LINES | keyof typeof EYE_GLYPHS;

/** Eyes drawn as a shape (happy, closed, star, heart, spiral) instead of the morphing ellipses. */
export function isDrawnEye(eyes: EyeShape): eyes is DrawnEye {
  return eyes in EYE_LINES || eyes in EYE_GLYPHS;
}

/** How a mouth part is painted: a line, a thinner line, the mouth's fill, or the tongue. */
export type MouthPaint = "line" | "thin" | "fill" | "tongue";

export type MouthPart = ({ kind: "path"; d: string } | ({ kind: "ellipse" } & Ellipse)) & { paint: MouthPaint };

const line = (d: string, paint: MouthPaint = "line"): MouthPart => ({ kind: "path", d, paint });
const blob = (cx: number, cy: number, rx: number, ry: number, paint: MouthPaint = "fill"): MouthPart => ({
  kind: "ellipse",
  cx,
  cy,
  rx,
  ry,
  paint,
});

/** Every mouth, back to front. */
export const MOUTHS: Record<Exclude<MouthShape, "none">, readonly MouthPart[]> = {
  smile: [line("M14.6 22.5q1.4 1.3 2.8 0")],
  grin: [line("M14.3 22.2h3.4q-.3 2.2-1.7 2.2t-1.7-2.2z", "fill"), blob(16, 23.7, 0.8, 0.4, "tongue")],
  laugh: [line("M13.9 21.9h4.2q-.3 3-2.1 3t-2.1-3z", "fill"), blob(16, 24.1, 1, 0.5, "tongue")],
  o: [blob(16, 23, 0.85, 1)],
  flat: [line("M15 23h2")],
  frown: [line("M14.7 23.8q1.3-1.3 2.6 0")],
  wobble: [line("M14.2 23.2q.45-.55.9 0t.9 0t.9 0t.9 0", "thin")],
  yawn: [blob(16, 23.3, 1.3, 1.8)],
  tongue: [line("M14.6 22.4q1.4 1.2 2.8 0"), line("M16.8 22.9q.2 1.3.9 1.1t.2-1.2z", "tongue")],
  cat: [line("M14.1 22.4q.95 1.1 1.9 0q.95 1.1 1.9 0")],
  smug: [line("M14.8 23q1.6.5 2.9-.9")],
  talk: [blob(16, 23, 0.95, 0.9)],
};

/** The sweat drop by its brow. */
export const SWEAT = "M24.3 11.4c.8 1.1 1.2 1.8 1.2 2.4a1.2 1.2 0 0 1-2.4 0c0-.6.4-1.3 1.2-2.4z";

/** A tear under each eye. */
export const TEARS: Record<Side, string> = {
  left: "M11.3 20.4c.5.7.8 1.2.8 1.6a.8.8 0 0 1-1.6 0c0-.4.3-.9.8-1.6z",
  right: "M20.7 20.4c.5.7.8 1.2.8 1.6a.8.8 0 0 1-1.6 0c0-.4.3-.9.8-1.6z",
};

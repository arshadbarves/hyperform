import type { Blush, BrowShape, EyeShape, MouthShape, PetFaceParts } from "./emotions";
import { MOUTHS, isDrawnEye, type DrawnEye, type Side } from "./faceShapes";
import { EASE, phase, sampleTrack, type Stop } from "./keyframes";
import type { PetStage } from "./looks";
import type { Point } from "./physics";

/**
 * How each face shape is posed, from pet-face.css: the ellipse eyes for
 * every eye shape, the blink, the brows, the blush, and the face's own
 * motion (parts popping in, the talking and yawning mouth, pulsing and
 * spinning drawn eyes, sweat and tears, the scanning gaze). Units are the
 * 32-unit space with y down; angles are radians, clockwise on screen as in
 * CSS and Canvas2D. The shapes themselves are in faceShapes.ts.
 */

/**
 * A face part's pose about its own centre: moved by (dx, dy), then turned
 * and scaled. In Canvas2D: translate(cx + dx, cy + dy), rotate(rot),
 * scale(sx, sy), translate(-cx, -cy).
 */
export interface PartTransform {
  dx: number;
  dy: number;
  rot: number;
  sx: number;
  sy: number;
}

export const IDENTITY: PartTransform = { dx: 0, dy: 0, rot: 0, sx: 1, sy: 1 };

type Sides = Record<Side, PartTransform>;

const deg = (d: number) => (d * Math.PI) / 180;
const part = (over: Partial<PartTransform>): PartTransform => ({ ...IDENTITY, ...over });
const sides = (left: Partial<PartTransform>, right = left): Sides => ({ left: part(left), right: part(right) });

/** Line widths the face is stroked with, in units, all with round caps (the mouth also with round joins). */
export const STROKE = { brow: 0.8, eyeLine: 1.15, spiral: 0.55, mouth: 0.9, mouthThin: 0.75 } as const;

const ELLIPSE_EYES: Record<Exclude<EyeShape, DrawnEye>, Sides> = {
  open: sides({}),
  wide: sides({ sx: 1.2, sy: 1.2 }),
  narrow: sides({ sy: 0.6 }),
  half: sides({ dy: 0.6, sy: 0.4 }),
  sad: sides({ dy: 0.4, sx: 0.95, sy: 0.8 }),
  puppy: sides({ sx: 1.3, sy: 1.25 }),
  uneven: sides({ sx: 1.16, sy: 1.16 }, { dy: 0.3, sy: 0.62 }),
};
const SEED_OPEN = sides({ sx: 1.12, sy: 1.12 });

export interface EyeLayout extends Sides {
  /** Drawn eyes (faceShapes' EYE_LINES / EYE_GLYPHS) replace the ellipses; their sides stay at rest. */
  drawn: boolean;
}

/** The ellipse eyes for a shape (a seed's open eyes are a little bigger), or that the eyes are drawn. */
export function eyeLayout(eyes: EyeShape, stage: PetStage = "sprout"): EyeLayout {
  if (isDrawnEye(eyes)) return { drawn: true, ...sides({}) };
  return { drawn: false, ...(eyes === "open" && stage === "seed" ? SEED_OPEN : ELLIPSE_EYES[eyes]) };
}

/** A blink squeezes the ellipse eyes to this, in place of their shape's own pose. */
export const BLINK: PartTransform = part({ sy: 0.1 });
/** How fast a blink closes and opens. */
export const BLINK_MS = 70;
/** Ellipse eyes morph from one shape to the next over this long (a snappy spring). */
export const EYE_MORPH_MS = 405;

/** The puppy eyes' glints (faceShapes' GLINTS) show unless it blinks. */
export function showsGlint(eyes: EyeShape, blinking: boolean): boolean {
  return eyes === "puppy" && !blinking;
}

export interface BrowLayout extends Sides {
  opacity: number;
}

const brows = (opacity: number, left: Partial<PartTransform>, right = left): BrowLayout => ({ opacity, ...sides(left, right) });

const BROWS_AT: Record<BrowShape, BrowLayout> = {
  none: brows(0, {}),
  raised: brows(0.85, { dy: -1.1 }),
  relaxed: brows(0.55, { dy: -0.4 }),
  worried: brows(0.85, { dy: -0.5, rot: deg(-15) }, { dy: -0.5, rot: deg(15) }),
  sad: brows(0.85, { dy: 0.2, rot: deg(-22) }, { dy: 0.2, rot: deg(22) }),
  firm: brows(0.85, { dy: 0.6, rot: deg(14) }, { dy: 0.6, rot: deg(-14) }),
  uneven: brows(0.85, { dy: -1.3, rot: deg(-8) }, { dy: 0.3, rot: deg(6) }),
};

/** Each brow's pose and how strongly the brows show (hidden unless the mood needs them). */
export function browLayout(shape: BrowShape): BrowLayout {
  return BROWS_AT[shape];
}

/** How strongly the cheeks blush, and how much they swell, about each cheek's centre. */
export function blushLayout(blush: Blush, stage: PetStage = "sprout"): { opacity: number; sx: number; sy: number } {
  if (blush === "strong") return { opacity: 0.85, sx: 1.18, sy: 1.1 };
  if (blush === "soft") return { opacity: 0.55, sx: 1, sy: 1 };
  return { opacity: stage === "bloom" || stage === "star" ? 0.34 : 0.22, sx: 1, sy: 1 };
}

/** How long a new part pops in: drawn eyes, and a mouth each time it changes. */
export const POP_MS = { eyes: 694, mouth: 405 } as const;

/** A drawn eye or a new mouth popping in, `tMs` after it appeared: its scale about its own centre, and opacity. */
export function popIn(kind: keyof typeof POP_MS, tMs: number): { scale: number; opacity: number } {
  const k = EASE.bouncy(phase(tMs, POP_MS[kind], 1) ?? 1);
  return { scale: 0.4 + 0.6 * k, opacity: Math.min(1, k) };
}

const TALK_MS = 340;
export const YAWN_MS = 1900;
const TALK: Stop[] = [
  [0, 0.55],
  [0.5, 1.15],
  [1, 0.55],
];
const YAWN_X: Stop[] = [
  [0, 0.5],
  [0.4, 1],
  [0.7, 1],
  [1, 0.5],
];
const YAWN_Y: Stop[] = [
  [0, 0.3],
  [0.4, 1.1],
  [0.7, 1.1],
  [1, 0.3],
];

/**
 * The mouth's own scale `tMs` after it appeared, about `pivot` (20% down
 * its ellipse): talking chatters, a yawn gapes once and then stays small
 * (CSS holds its last frame). Every other mouth is still.
 */
export function mouthScale(mouth: MouthShape, tMs: number): { sx: number; sy: number; pivot: Point } {
  const first = mouth === "none" ? null : MOUTHS[mouth][0];
  const pivot = first?.kind === "ellipse" ? { x: first.cx, y: first.cy - first.ry * 0.6 } : { x: 16, y: 23 };
  if (mouth === "talk") return { sx: 1, sy: sampleTrack(TALK, phase(tMs, TALK_MS) ?? 0, EASE.inOut), pivot };
  if (mouth !== "yawn") return { sx: 1, sy: 1, pivot };
  const u = phase(tMs, YAWN_MS, 1) ?? 1;
  return { sx: sampleTrack(YAWN_X, u, EASE.inOut), sy: sampleTrack(YAWN_Y, u, EASE.inOut), pivot };
}

const PULSE: Stop[] = [
  [0, 1],
  [0.5, 1.16],
  [1, 1],
];

/** Star and heart eyes pulse once they have popped in; spiral eyes spin. Scale and turn about each eye's centre, `tMs` after they appeared. */
export function drawnEyeMotion(eyes: EyeShape, tMs: number): { scale: number; rot: number } {
  if (eyes === "star" || eyes === "heart") {
    return { scale: sampleTrack(PULSE, phase(tMs - POP_MS.eyes, 900) ?? 0, EASE.inOut), rot: 0 };
  }
  if (eyes === "spiral") return { scale: 1, rot: 2 * Math.PI * (phase(tMs, 1100) ?? 0) };
  return { scale: 1, rot: 0 };
}

export const SWEAT_MS = 2400;
export const TEAR_MS = 1800;
/** The right tear starts this much after the left. */
export const TEAR_LATE_MS = 700;
const DRIP_OPACITY: Stop[] = [
  [0, 0],
  [0.2, 0.9],
  [0.6, 0.9],
  [1, 0],
];
const DRIP_Y: Stop[] = [
  [0, -0.6],
  [1, 2.4],
];

/** A drop sliding down (dy, units) and fading, over and over. */
export interface Drip {
  opacity: number;
  dy: number;
}

function drip(tMs: number, ms: number): Drip {
  const u = phase(tMs, ms) ?? 0;
  return { opacity: sampleTrack(DRIP_OPACITY, u, EASE.in), dy: sampleTrack(DRIP_Y, u, EASE.in) };
}

/** The sweat drop, `tMs` after it appeared. */
export function sweatDrip(tMs: number): Drip {
  return drip(tMs, SWEAT_MS);
}

/** Each tear, `tMs` after they appeared. */
export function tearDrip(side: Side, tMs: number): Drip {
  return drip(side === "right" ? tMs - TEAR_LATE_MS : tMs, TEAR_MS);
}

const SCAN: Stop[] = [
  [0, 0],
  [0.25, -1.8],
  [0.75, 1.8],
  [1, 0],
];

/** The eyes sweeping side to side while the gaze is "scan" (faster while searching), added to their gaze offset. */
export function scanOffset(tMs: number, searching = false): Point {
  return { x: sampleTrack(SCAN, phase(tMs, searching ? 900 : 1600) ?? 0, EASE.inOut), y: 0 };
}

/**
 * Whether the face still moves `sinceMs` after it last changed: its
 * loops (a talking mouth, star, heart or spiral eyes, sweat, tears) never
 * stop; pop-ins and a yawn run out. The scanning gaze is the caller's.
 */
export function faceAnimates(face: PetFaceParts, sinceMs: number): boolean {
  const loops = face.mouth === "talk" || face.eyes === "star" || face.eyes === "heart" || face.eyes === "spiral";
  if (loops || face.sweat || face.tears) return true;
  return sinceMs < Math.max(POP_MS.eyes, face.mouth === "yawn" ? YAWN_MS : 0);
}

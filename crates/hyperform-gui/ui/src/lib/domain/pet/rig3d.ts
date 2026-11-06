import type { BodyMotion } from "./emotions";
import { EASE, phase, sampleTrack, type Ease, type Stop } from "./keyframes";
import type { PetMood } from "./pose";
import type { PetRoutine } from "./routines";

/**
 * The pet's body motions (pet-moves.css, pet-keyframes.css) as a 3D rig
 * pose per frame: each mood's body motion, on top of it the idle routine,
 * and the aura's glow. The 2D rolls (sway, wiggle, wave, doze, lean,
 * tilt) stay rolls; a nod pitches forward, looking around and the spin
 * trick turn the body.
 *
 * Axes: the pet's 32-unit space with x right, y down (as in the SVG) and
 * z toward the viewer. Everything pivots on the feet (16, 28), and a yaw
 * turns about the vertical axis through them. Radians: rotX > 0 tips the
 * top toward the viewer, rotY > 0 turns the front toward screen right,
 * rotZ > 0 rolls clockwise on screen (CSS `rotate`). `threeAxes` converts
 * to Three.js (y up).
 *
 * Breathing stays a CSS loop on the wrapper around the canvas, so
 * "breathe" (and the breathing under lean, tilt, slump and sleep) is not
 * in the rig. A hop is the motion engine's own, and the wave and yawn
 * routines play through their mood (greeting, yawning; see behavior.ts).
 */

export interface RigPose {
  x: number;
  y: number;
  z: number;
  rotX: number;
  rotY: number;
  rotZ: number;
  sx: number;
  sy: number;
  sz: number;
}

export interface BodyPose extends RigPose {
  /** The aura's strength: 1 at rest, glowing from 2/3 to 8/3 of it (CSS opacity 0.2..0.8 over its resting 0.3). */
  aura: number;
  /** How high each foot [left, right] is lifted, in units (up is positive, as the motion engine's `lift`). */
  lift: [number, number];
}

export const REST_POSE: BodyPose = { x: 0, y: 0, z: 0, rotX: 0, rotY: 0, rotZ: 0, sx: 1, sy: 1, sz: 1, aura: 1, lift: [0, 0] };

type Channel = "x" | "y" | "rotX" | "rotY" | "rotZ" | "sx" | "sy" | "liftR" | "aura";
type Layer = Partial<Record<Channel, number>>;

/** One CSS animation: `ms` per iteration, played `times` times (a loop by default). */
interface Motion {
  ms: number;
  ease: Ease;
  times?: number;
  tracks: Partial<Record<Channel, readonly Stop[]>>;
}

const deg = (d: number) => (d * Math.PI) / 180;

function loop(ms: number, ease: Ease, tracks: Motion["tracks"], times?: number): Motion {
  return { ms, ease, tracks, times };
}

/** A roll to -`d`° and then `d`°, and back. */
function roll(ms: number, d: number, times?: number): Motion {
  return loop(ms, EASE.inOut, { rotZ: [[0, 0], [0.25, deg(-d)], [0.75, deg(d)], [1, 0]] }, times);
}

/** Squash and stretch through `keys` of [at, sx, sy], from round and back. */
function squash(ms: number, ease: Ease, keys: readonly (readonly [number, number, number])[], times?: number): Motion {
  const sx: Stop[] = [[0, 1], ...keys.map(([at, x]): Stop => [at, x]), [1, 1]];
  const sy: Stop[] = [[0, 1], ...keys.map(([at, , y]): Stop => [at, y]), [1, 1]];
  return loop(ms, ease, { sx, sy }, times);
}

/** The aura's glow (pet-glow's opacity 0.2..0.8), as a share of its resting 0.3. */
function glow(ms: number, times?: number): Motion {
  const [dim, bright] = [0.2 / 0.3, 0.8 / 0.3];
  return loop(ms, EASE.inOut, { aura: [[0, dim], [0.5, bright], [1, dim]] }, times);
}

/** The still part of a mood's body, under its loop. */
const HOLD: Partial<Record<BodyMotion, Layer>> = {
  lean: { x: 0.6, rotZ: deg(7) },
  tilt: { rotZ: deg(-9) },
  puff: { sx: 1.05, sy: 1.03 },
  slump: { y: 1, sx: 1.05, sy: 0.92 },
  tremble: { y: 0.6, sy: 0.96 },
  sink: { y: 5 },
  sleep: { y: 1.2, sx: 1.06, sy: 0.9 },
};

const LOOPS: Partial<Record<BodyMotion, Motion>> = {
  bob: loop(1100, EASE.inOut, { y: [[0, 0], [0.5, -1.2], [1, 0]] }),
  sway: roll(3000, 4),
  nod: loop(2400, EASE.inOut, {
    rotX: [[0, 0], [0.6, 0], [0.75, deg(5)], [1, 0]],
    y: [[0, 0], [0.6, 0], [0.75, 0.5], [1, 0]],
  }),
  type: loop(320, EASE.inOut, { y: [[0, 0], [0.5, -0.45], [1, 0]] }),
  sweep: roll(900, 4),
  bounce: loop(1500, EASE.out, {
    y: [[0, 0], [0.3, -3.2], [0.46, 0], [0.55, 0], [1, 0]],
    sx: [[0, 1], [0.12, 1.08], [0.3, 0.95], [0.46, 1.07], [0.55, 1], [1, 1]],
    sy: [[0, 1], [0.12, 0.9], [0.3, 1.06], [0.46, 0.92], [0.55, 1], [1, 1]],
  }),
  puff: squash(2800, EASE.inOut, [[0.4, 1.03, 1.04]]),
  sigh: squash(3200, EASE.inOut, [[0.35, 0.98, 1.05], [0.7, 1.04, 0.93]]),
  tremble: loop(1800, EASE.linear, {
    x: [[0, 0], [0.06, -0.35], [0.12, 0.35], [0.18, -0.35], [0.24, 0.35], [0.3, -0.35], [0.36, 0], [1, 0]],
  }),
  squish: squash(1300, EASE.inOut, [[0.3, 1.06, 0.94], [0.6, 0.97, 1.04]]),
  wiggle: roll(800, 6),
  wave: roll(1200, 12, 2),
  yawn: squash(1900, EASE.inOut, [[0.45, 0.96, 1.08], [0.7, 0.96, 1.08]], 1),
  doze: loop(4400, EASE.inOut, {
    rotZ: [[0, 0], [0.55, deg(7)], [0.62, deg(-1)], [1, 0]],
    y: [[0, 0], [0.55, 0.8], [0.62, 0], [1, 0]],
  }),
};

/** Looking around turns this far to each side (the SVG's 5° roll reads as a proper glance in 3D). */
const LOOK_YAW = deg(25);

const ROUTINE_HOLD: Partial<Record<PetRoutine, Layer>> = {
  sit: { y: 0.8, sx: 1.05, sy: 0.94 },
  lieDown: { y: 2.2, sx: 1.14, sy: 0.8 },
};

const ROUTINES: Partial<Record<PetRoutine, Motion>> = {
  stretch: squash(1400, EASE.bouncy, [[0.4, 0.9, 1.14], [0.7, 1.05, 0.96]], 1),
  spin: loop(1000, EASE.gentle, { rotY: [[0, 0], [1, 2 * Math.PI]] }, 1),
  sparkle: glow(1400, 1),
  lookAround: loop(
    2600,
    EASE.inOut,
    { rotY: [[0, 0], [0.25, -LOOK_YAW], [0.4, -LOOK_YAW], [0.6, LOOK_YAW], [0.78, LOOK_YAW], [1, 0]] },
    1,
  ),
  tap: loop(450, EASE.inOut, { liftR: [[0, 0], [0.5, 1.1], [1, 0]] }, 4),
};

/** The big feelings make the aura pulse. */
const MOOD_GLOW: Partial<Record<PetMood, Motion>> = { excited: glow(1200), proud: glow(1200), love: glow(1200) };

function sample(motion: Motion | undefined, tMs: number): Layer {
  const u = motion ? phase(tMs, motion.ms, motion.times) : null;
  if (!motion || u === null) return {};
  const out: Layer = {};
  for (const [channel, stops] of Object.entries(motion.tracks) as [Channel, readonly Stop[]][]) {
    out[channel] = sampleTrack(stops, u, motion.ease);
  }
  return out;
}

function compose(layers: Layer[]): BodyPose {
  const at: Record<Channel, number> = { x: 0, y: 0, rotX: 0, rotY: 0, rotZ: 0, sx: 1, sy: 1, liftR: 0, aura: 1 };
  for (const layer of layers) {
    for (const [channel, value] of Object.entries(layer) as [Channel, number][]) {
      if (channel === "sx" || channel === "sy") at[channel] *= value;
      else if (channel === "aura") at.aura = value;
      else at[channel] += value;
    }
  }
  const { x, y, rotX, rotY, rotZ, sx, sy, liftR, aura } = at;
  return { x, y, z: 0, rotX, rotY, rotZ, sx, sy, sz: sx, aura, lift: [0, liftR] };
}

export interface RigOptions {
  /** Ms since the routine started, when it did not start with the body motion. */
  routineMs?: number;
  /** The mood, so the big feelings' aura pulses (the sparkle trick's glow wins while it plays). */
  mood?: PetMood;
}

/**
 * The body's pose `tMs` after its motion started (one-shots such as the
 * wave and yawn count from there), with the idle `routine` on top.
 * Translations add, rotations add and scales multiply; the 2D horizontal
 * squash also squashes depth (sz follows sx).
 */
export function bodyPose(body: BodyMotion, routine: PetRoutine | null, tMs: number, opts: RigOptions = {}): BodyPose {
  const routineMs = opts.routineMs ?? tMs;
  return compose([
    HOLD[body] ?? {},
    sample(LOOPS[body], tMs),
    opts.mood ? sample(MOOD_GLOW[opts.mood], tMs) : {},
    routine ? (ROUTINE_HOLD[routine] ?? {}) : {},
    routine ? sample(ROUTINES[routine], routineMs) : {},
  ]);
}

function running(motion: Motion | undefined, tMs: number | undefined): boolean {
  return !!motion && (tMs === undefined || phase(tMs, motion.ms, motion.times) !== null);
}

/**
 * Whether the pose changes over time, so the renderer needs frames: at
 * `tMs` (and `opts.routineMs`) when given, one-shots stop counting once
 * they end; without a time they count. Still poses never need frames.
 */
export function animates(body: BodyMotion, routine: PetRoutine | null, tMs?: number, opts: RigOptions = {}): boolean {
  const routineMs = opts.routineMs ?? tMs;
  return (
    running(LOOPS[body], tMs) ||
    (!!routine && running(ROUTINES[routine], routineMs)) ||
    (!!opts.mood && running(MOOD_GLOW[opts.mood], tMs))
  );
}

/** The seed is this much smaller than the grown pet, about its feet (pet-moves.css `.stage-seed`). */
export const SEED_SCALE = 0.86;

/** Turning to face its way (the motion engine's facing, -1 left .. 1 right, 0 mid-turn) is a yaw this far each side. */
export const FACE_YAW = deg(30);

/** The body's yaw for the motion engine's continuous facing: 0 faces the viewer, ±1 turns FACE_YAW to that side. */
export function faceYaw(face: number): number {
  if (!Number.isFinite(face)) return 0;
  return Math.min(1, Math.max(-1, face)) * FACE_YAW;
}

/** A pose in Three.js axes (y up, right-handed): y and the roll flip sign. */
export function threeAxes(pose: RigPose): {
  position: [number, number, number];
  rotation: [number, number, number];
  scale: [number, number, number];
} {
  return {
    position: [pose.x, -pose.y, pose.z],
    rotation: [pose.rotX, pose.rotY, -pose.rotZ],
    scale: [pose.sx, pose.sy, pose.sz],
  };
}

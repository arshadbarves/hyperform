import type { Facing, MoveFrame } from "./motion";
import {
  PET_SPRINGS,
  capSpeed,
  countSwing,
  releaseVelocity,
  stepPendulum,
  stepSpring,
  type Body2D,
  type Pendulum,
  type Point,
  type Sample,
  type SwingCount,
} from "./physics";

/**
 * The pet in your hand: its feet follow the pointer on a spring, it swings
 * from your grip like a pendulum, stretches as it hangs and kicks its
 * feet; let go, it keeps the pointer's speed. Viewport pixels and ms.
 */
export interface Held {
  body: Body2D;
  /** Where the feet should be: the pointer less the grip. */
  aim: Point;
  /** The pointer from the feet when it was picked up. */
  offset: Point;
  swing: Pendulum;
  count: SwingCount;
  samples: readonly Sample[];
}

/** How it hangs this frame; `facing` is set once it moves sideways fast enough to turn. */
export type HeldFrame = Omit<MoveFrame, "size" | "phase"> & { facing: Facing | null };

/** Thrown faster than this (px/ms) counts as this fast. */
export const MAX_THROW = 3.2;
/** Thrown this hard, or swung wildly this many times, it lands dizzy. */
export const DIZZY_SPEED = 2.2;
export const DIZZY_SWINGS = 3;
const SAMPLES = 12;
/** Sideways speed (px/ms) at which it turns to face where it is carried. */
const TURN_SPEED = 0.35;

export function grab(feet: Point, pointer: Point, t: number): Held {
  return {
    body: { x: feet.x, y: feet.y, vx: 0, vy: 0 },
    aim: { x: feet.x, y: feet.y },
    offset: { x: pointer.x - feet.x, y: pointer.y - feet.y },
    swing: { angle: 0, speed: 0 },
    count: { side: 0, wild: 0 },
    samples: [{ ...pointer, t }],
  };
}

/** The pointer moved to `pointer` at `t`. */
export function pull(held: Held, pointer: Point, t: number): Held {
  const aim = { x: pointer.x - held.offset.x, y: pointer.y - held.offset.y };
  return { ...held, aim, samples: [...held.samples, { ...pointer, t }].slice(-SAMPLES) };
}

/** Advances a held pet `size` px tall by `dt` ms. */
export function stepHeld(held: Held, dt: number, now: number, size: number): { held: Held; frame: HeldFrame } {
  const body = stepSpring(held.body, held.aim, PET_SPRINGS.follow, dt);
  const ax = dt > 0 ? (body.vx - held.body.vx) / dt : 0;
  const swing = stepPendulum(held.swing, ax, size * 0.6, dt);
  const hang = 0.05 + Math.min(0.08, Math.abs(body.vy) * 0.08);
  const kick = Math.sin(now / 110) * 0.9;
  const facing: Facing | null = Math.abs(body.vx) > TURN_SPEED ? (body.vx > 0 ? 1 : -1) : null;
  return {
    held: { ...held, body, swing, count: countSwing(held.count, swing.angle) },
    frame: {
      x: body.x,
      y: body.y,
      sx: 1 - hang * 0.7,
      sy: 1 + hang,
      rot: (-swing.angle * 180) / Math.PI,
      lift: [kick - 0.6, -kick - 0.6],
      bob: 0,
      facing,
    },
  };
}

/**
 * Where its feet are drawn while it is rotated (`rot` degrees) and
 * stretched about the grip, so the pivot can move back to the feet
 * without a jump.
 */
export function drawnFeet(feet: Point, offset: Point, rot: number, sx: number, sy: number): Point {
  const a = (rot * Math.PI) / 180;
  const ox = offset.x * sx;
  const oy = offset.y * sy;
  return {
    x: feet.x + offset.x - (ox * Math.cos(a) - oy * Math.sin(a)),
    y: feet.y + offset.y - (ox * Math.sin(a) + oy * Math.cos(a)),
  };
}

/** Let go at `now`: the throw's velocity (capped) and whether it was wild enough to leave it dizzy. */
export function letGo(held: Held, now: number): { v: Point; speed: number; dizzy: boolean } {
  const v = capSpeed(releaseVelocity(held.samples, now), MAX_THROW);
  const speed = Math.hypot(v.x, v.y);
  return { v, speed, dizzy: speed > DIZZY_SPEED || held.count.wild >= DIZZY_SWINGS };
}

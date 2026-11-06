/**
 * The pet's physics, in viewport pixels and milliseconds: a 2D spring with
 * velocity (it follows the pointer while dragged), a pendulum (it swings
 * from where you hold it) and the pointer's speed when you let go.
 */

export interface Point {
  x: number;
  y: number;
}

/** A body on a spring: where it is and how fast it moves, in px per ms. */
export interface Body2D extends Point {
  vx: number;
  vy: number;
}

/** A spring in per-millisecond units: `stiffness` in 1/ms², `damping` in 1/ms. */
export interface SpringTune {
  stiffness: number;
  damping: number;
}

/** Everything falls at this rate, in px/ms² (about 3,600 px/s²). */
export const GRAVITY = 0.0036;
/** Longest step the integrators take at once; larger frames are split. */
const MAX_STEP_MS = 8;

/** A spring tuned the way designers do: `responseMs` is its period, `bounce` 0 is critically damped, up to 1 is springier. */
export function tune(responseMs: number, bounce = 0): SpringTune {
  const omega = (2 * Math.PI) / Math.max(responseMs, 1);
  const ratio = 1 - Math.min(Math.max(bounce, 0), 0.95);
  return { stiffness: omega * omega, damping: 2 * ratio * omega };
}

/** Held in the hand it lags a little behind the pointer; nudged on its perch it settles softly. */
export const PET_SPRINGS = {
  follow: tune(110, 0.15),
  settle: tune(320, 0.3),
} as const;

/** Advances `body` toward `target` by `dt` ms (semi-implicit Euler, split into short steps). */
export function stepSpring(body: Body2D, target: Point, spring: SpringTune, dt: number): Body2D {
  const steps = Math.max(1, Math.ceil(dt / MAX_STEP_MS));
  const h = dt / steps;
  let { x, y, vx, vy } = body;
  for (let i = 0; i < steps; i++) {
    vx += (-spring.stiffness * (x - target.x) - spring.damping * vx) * h;
    vy += (-spring.stiffness * (y - target.y) - spring.damping * vy) * h;
    x += vx * h;
    y += vy * h;
  }
  return { x, y, vx, vy };
}

/** Close enough to `target` and slow enough to stop animating. */
export function atRest(body: Body2D, target: Point, distance = 0.25, speed = 0.005): boolean {
  return Math.hypot(body.x - target.x, body.y - target.y) < distance && Math.hypot(body.vx, body.vy) < speed;
}

/** A pet hanging from the pointer: its angle from straight down (radians, positive swings its feet to +x) and angular speed (rad/ms). */
export interface Pendulum {
  angle: number;
  speed: number;
}

const SWING_DAMPING = 0.0045;
/** It never swings past this, however hard it is shaken. */
export const MAX_SWING = 1.1;

/**
 * Advances a hanging pet by `dt` ms. `ax` is how fast the grip speeds up
 * sideways (px/ms²): moving the hand right swings the feet left, and
 * gravity rights it again. `length` is from the grip to its middle.
 */
export function stepPendulum(p: Pendulum, ax: number, length: number, dt: number): Pendulum {
  const steps = Math.max(1, Math.ceil(dt / MAX_STEP_MS));
  const h = dt / steps;
  const l = Math.max(length, 4);
  let { angle, speed } = p;
  for (let i = 0; i < steps; i++) {
    const accel = -(GRAVITY / l) * Math.sin(angle) - (ax / l) * Math.cos(angle) - SWING_DAMPING * speed;
    speed += accel * h;
    angle += speed * h;
    if (Math.abs(angle) > MAX_SWING) {
      angle = Math.sign(angle) * MAX_SWING;
      speed *= -0.3;
    }
  }
  return { angle, speed };
}

/** A pointer position at a time, for the release speed. */
export interface Sample extends Point {
  t: number;
}

/** Samples older than this don't count toward the release speed. */
export const VELOCITY_WINDOW_MS = 90;

/** The pointer's velocity (px/ms) over the last `window` ms before `now`; zero when it stopped or there is too little to tell. */
export function releaseVelocity(samples: readonly Sample[], now: number, window = VELOCITY_WINDOW_MS): Point {
  const recent = samples.filter((s) => now - s.t <= window);
  if (recent.length < 2) return { x: 0, y: 0 };
  const first = recent[0];
  const last = recent[recent.length - 1];
  const span = last.t - first.t;
  if (span <= 0) return { x: 0, y: 0 };
  return { x: (last.x - first.x) / span, y: (last.y - first.y) / span };
}

/** `v` shortened to at most `max` px/ms, keeping its direction. */
export function capSpeed(v: Point, max: number): Point {
  const speed = Math.hypot(v.x, v.y);
  if (speed <= max || speed === 0) return v;
  return { x: (v.x / speed) * max, y: (v.y / speed) * max };
}

/** A swing counts as wild when it reaches this far out (radians) on one side and then on the other. */
export const WILD_SWING = 0.55;

/** Wild swings so far while held, and the side it last swung out to (-1, 0 or 1). */
export interface SwingCount {
  side: -1 | 0 | 1;
  wild: number;
}

/** Counts a wild swing each time the pet swings far out to the other side. */
export function countSwing(count: SwingCount, angle: number): SwingCount {
  const side = angle > WILD_SWING ? 1 : angle < -WILD_SWING ? -1 : 0;
  if (side === 0 || side === count.side) return count;
  return { side, wild: count.side === 0 ? count.wild : count.wild + 1 };
}

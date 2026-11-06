import type { PetSpot } from "./perches";
import { GRAVITY, type Point } from "./physics";

/**
 * How the pet gets from one spot to another: `planMove` walks it along an
 * edge or hops it between perches, `planFling` / `planDrop` land it after
 * you let go, `retargetMove` follows a perch that scrolls, and `sampleMove`
 * says where it is and how squashed it looks. Pure: petMotion.svelte.ts
 * plays the frames.
 */

export type MoveKind = "walk" | "hop" | "fling" | "drop";
export type MovePhase = "windup" | "air" | "walk" | "land" | "done";
export type Facing = 1 | -1;

/** One of a drop's little bounces after it lands. */
export type Bounce = { height: number; ms: number };

export interface MovePlan {
  kind: MoveKind;
  from: PetSpot;
  to: PetSpot;
  /** The flight's quadratic Bézier control point (hop, fling). */
  control: Point;
  /** Before it leaves: crouching for a hop, turning around for a walk. */
  windup: number;
  /** In the air, or walking. */
  travel: number;
  /** Landing: the squash, and a drop's bounces. */
  land: number;
  /** Footsteps in a walk, always even so it ends on both feet. */
  steps: number;
  /** How hard it lands, 0..1. */
  impact: number;
  bounces: readonly Bounce[];
  facing: Facing;
  /** Degrees it turns while flying: a hard fling flips it over. */
  spin: number;
}

export interface MoveFrame extends PetSpot {
  /** Squash and stretch: sy above 1 is taller. */
  sx: number;
  sy: number;
  /** Lean in degrees, clockwise. */
  rot: number;
  /** How high each foot is lifted, in the pet's own 32-unit space. */
  lift: [number, number];
  /** The body's bob in the pet's units; negative is up. */
  bob: number;
  phase: MovePhase;
}

/** How long turning around takes. */
export const TURN_MS = 170;
const HOP_WINDUP_MS = 120;
const LAND_MS = 300;
const WALK_LAND_MS = 150;
/** Longer strolls along an edge become a hop. */
export const WALK_MAX = 420;
/** Closer than this, it just shuffles into place without a move. */
const MIN_MOVE = 2.5;
/** A walk speeds up and slows down over this share of its time at each end. */
const RAMP = 0.18;
const STEP_LIFT = 1.7;
const STEP_BOB = 1.1;
/** Released faster than this (px/ms), it flies; slower, it drops. */
export const FLING_SPEED = 0.45;
/** Flung this hard, it flips over in the air. */
const FLIP_SPEED = 2.2;
/** Where a fling is aimed: this many ms along its release velocity. */
export const FLING_LOOKAHEAD_MS = 240;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const smooth = (t: number) => t * t * (3 - 2 * t);

function plan(kind: MoveKind, from: PetSpot, to: PetSpot, over: Partial<MovePlan>): MovePlan {
  return {
    kind,
    from,
    to,
    control: { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 },
    windup: 0,
    travel: 0,
    land: LAND_MS,
    steps: 0,
    impact: 0.3,
    bounces: [],
    facing: 1,
    spin: 0,
    ...over,
  };
}

/** Walking speed in px/ms: bigger pets stride faster. */
export function walkSpeed(size: number): number {
  return clamp(size * 0.0032, 0.09, 0.2);
}

function walk(from: PetSpot, to: PetSpot, facing: Facing, turn: boolean): MovePlan {
  const dist = Math.abs(to.x - from.x);
  const stride = Math.max(8, to.size * 0.42);
  const steps = Math.max(2, 2 * Math.round(dist / stride / 2));
  const travel = dist / (walkSpeed(to.size) * (1 - RAMP));
  return plan("walk", from, to, { windup: turn ? TURN_MS : 0, travel, land: WALK_LAND_MS, steps, impact: 0.12, facing });
}

/** How hard it lands at `speed` px/ms. */
function impactFor(speed: number): number {
  return clamp(speed / 1.6, 0.25, 1);
}

/** A ballistic hop that clears the higher end by `lift` px, keeping its head below `top`. */
function hop(from: PetSpot, to: PetSpot, facing: Facing, turn: boolean, top: number, lift?: number): MovePlan {
  const dist = Math.hypot(to.x - from.x, to.y - from.y);
  const higher = Math.min(from.y, to.y);
  const rise = lift ?? clamp(dist * 0.3, to.size * 0.55, 130);
  const apex = Math.min(higher - 4, Math.max(top, higher - rise));
  const up = from.y - apex;
  const down = to.y - apex;
  const h = (Math.sqrt(up) + Math.sqrt(down)) ** 2 / 4;
  const control = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 - 2 * h };
  const travel = clamp(Math.sqrt((2 * up) / GRAVITY) + Math.sqrt((2 * down) / GRAVITY), 220, 820);
  const windup = Math.max(HOP_WINDUP_MS, turn ? TURN_MS : 0);
  return plan("hop", from, to, { control, windup, travel, impact: impactFor(Math.sqrt(2 * GRAVITY * down)), facing });
}

export interface MoveOptions {
  /** It stays on the same edge (a stroll), so it may walk. */
  sameEdge: boolean;
  /** Which way it faces now. */
  facing: Facing;
  /** The highest its feet may go: its head stays in the window. */
  top?: number;
}

/** The move from `from` to `to`, or null when it is close enough to just step there. */
export function planMove(from: PetSpot, to: PetSpot, options: MoveOptions): MovePlan | null {
  const dx = to.x - from.x;
  const dist = Math.hypot(dx, to.y - from.y);
  if (dist < MIN_MOVE && Math.abs(to.size - from.size) < 0.5) return null;
  const facing: Facing = Math.abs(dx) > 1 ? (dx > 0 ? 1 : -1) : options.facing;
  const turn = facing !== options.facing;
  if (options.sameEdge && Math.abs(to.y - from.y) < 6 && dist <= WALK_MAX) return walk(from, to, facing, turn);
  return hop(from, to, facing, turn, options.top ?? to.size);
}

/** A hop on the spot, `height` px high: for joy, a trick or a greeting. */
export function planHop(at: PetSpot, height: number, facing: Facing): MovePlan {
  return hop(at, at, facing, false, -Infinity, height);
}

/** Where a fling released at `from` with velocity `v` (px/ms) is heading. */
export function flingAim(from: Point, v: Point): Point {
  return { x: from.x + v.x * FLING_LOOKAHEAD_MS, y: from.y + v.y * FLING_LOOKAHEAD_MS };
}

/**
 * Let go mid-drag: it keeps its speed and curves onto `to`. The flight is
 * a Bézier whose first tangent is the release velocity, so there is no
 * jolt; `bounds` keeps the curve in the window.
 */
export function planFling(from: PetSpot, v: Point, to: PetSpot, bounds: { width: number; height: number }): MovePlan {
  const speed = Math.hypot(v.x, v.y);
  const dist = Math.hypot(to.x - from.x, to.y - from.y);
  const travel = clamp((1.25 * dist) / Math.max(speed, 0.8), 320, 900);
  const control = {
    x: clamp(from.x + (v.x * travel) / 2, 0, bounds.width),
    y: clamp(from.y + (v.y * travel) / 2, to.size, bounds.height),
  };
  const endSpeed = (2 * Math.hypot(to.x - control.x, to.y - control.y)) / travel;
  const facing: Facing = (Math.abs(v.x) > 0.05 ? v.x : to.x - from.x) < 0 ? -1 : 1;
  const spin = speed > FLIP_SPEED ? 360 * facing : 0;
  return plan("fling", from, to, { control, travel, impact: impactFor(endSpeed), facing, spin });
}

/** Let go gently above a perch: it falls under gravity and bounces a little. Onto a spot above it, it hops instead. */
export function planDrop(from: PetSpot, to: PetSpot, facing: Facing): MovePlan {
  const fall = to.y - from.y;
  if (fall < 12) return hop(from, to, facing, false, to.size);
  const travel = Math.sqrt((2 * fall) / GRAVITY);
  const first = Math.min(fall * 0.14, to.size * 0.6);
  const bounces = [first, first * 0.28]
    .filter((height) => height > 1.5)
    .map((height) => ({ height, ms: 2 * Math.sqrt((2 * height) / GRAVITY) }));
  const land = bounces.reduce((sum, b) => sum + b.ms, 0) + LAND_MS;
  return plan("drop", from, to, { travel, land, bounces, impact: impactFor(GRAVITY * travel), facing });
}

/** A move under way, onto `to` after its perch moved: a walk shifts with its edge, a flight bends its end. */
export function retargetMove(p: MovePlan, to: PetSpot): MovePlan {
  const d = { x: to.x - p.to.x, y: to.y - p.to.y };
  const from = p.kind === "walk" ? { ...p.from, x: p.from.x + d.x, y: p.from.y + d.y } : p.from;
  return { ...p, from, to, control: { x: p.control.x + d.x, y: p.control.y + d.y } };
}

export function moveDuration(p: MovePlan): number {
  return p.windup + p.travel + p.land;
}

function frame(at: Point, size: number, phase: MovePhase, over: Partial<MoveFrame> = {}): MoveFrame {
  return { x: at.x, y: at.y, size, sx: 1, sy: 1, rot: 0, lift: [0, 0], bob: 0, phase, ...over };
}

function bezier(p: MovePlan, u: number): Point {
  const a = (1 - u) * (1 - u);
  const b = 2 * (1 - u) * u;
  const c = u * u;
  return { x: a * p.from.x + b * p.control.x + c * p.to.x, y: a * p.from.y + b * p.control.y + c * p.to.y };
}

/** Stretch along the travel for a speed in px/ms. */
function stretch(speed: number): Pick<MoveFrame, "sx" | "sy"> {
  const sy = 1 + Math.min(0.16, speed * 0.15);
  return { sx: 1 - (sy - 1) * 0.7, sy };
}

function windupFrame(p: MovePlan, u: number): MoveFrame {
  if (p.kind !== "hop") return frame(p.from, p.from.size, "windup");
  const crouch = Math.sin((u * Math.PI) / 2);
  return frame(p.from, p.from.size, "windup", { sx: 1 + 0.13 * crouch, sy: 1 - 0.17 * crouch });
}

function flightFrame(p: MovePlan, u: number): MoveFrame {
  const at = bezier(p, u);
  const vx = (2 * (1 - u) * (p.control.x - p.from.x) + 2 * u * (p.to.x - p.control.x)) / p.travel;
  const vy = (2 * (1 - u) * (p.control.y - p.from.y) + 2 * u * (p.to.y - p.control.y)) / p.travel;
  const lean = clamp(vx * 14, -16, 16) * Math.sin(Math.PI * u);
  const tuck = 1.3 * Math.sqrt(Math.sin(Math.PI * u));
  const rot = lean + p.spin * smooth(u);
  const size = lerp(p.from.size, p.to.size, smooth(u));
  return frame(at, size, "air", { ...stretch(Math.hypot(vx, vy)), rot, lift: [tuck, tuck] });
}

function fallFrame(p: MovePlan, ms: number): MoveFrame {
  const u = ms / p.travel;
  const y = Math.min(p.to.y, p.from.y + 0.5 * GRAVITY * ms * ms);
  const at = { x: lerp(p.from.x, p.to.x, u), y };
  return frame(at, lerp(p.from.size, p.to.size, smooth(u)), "air", { ...stretch(GRAVITY * ms), lift: [-0.5, -0.5] });
}

/** Share of a walk's distance covered at `u` of its time: speeding up, cruising, slowing down. */
function walked(u: number): number {
  if (u < RAMP) return (u * u) / (2 * RAMP) / (1 - RAMP);
  if (u > 1 - RAMP) return 1 - (1 - u) ** 2 / (2 * RAMP) / (1 - RAMP);
  return (u - RAMP / 2) / (1 - RAMP);
}

function walkFrame(p: MovePlan, u: number): MoveFrame {
  const along = walked(u);
  const pace = u < RAMP ? u / RAMP : u > 1 - RAMP ? (1 - u) / RAMP : 1;
  const stepAt = along * p.steps;
  const step = Math.floor(stepAt);
  const swing = Math.sin(Math.PI * (stepAt - step));
  const lift: [number, number] = step % 2 === 0 ? [STEP_LIFT * swing, 0] : [0, STEP_LIFT * swing];
  const at = { x: lerp(p.from.x, p.to.x, along), y: lerp(p.from.y, p.to.y, along) };
  return frame(at, p.to.size, "walk", {
    sx: 1 - 0.02 * swing,
    sy: 1 + 0.03 * swing,
    rot: p.facing * 4 * pace,
    lift,
    bob: -STEP_BOB * swing,
  });
}

/** A landing squash `ms` after touching down with `impact`: it squashes, rebounds a little and settles. */
export function landSquash(ms: number, impact: number): Pick<MoveFrame, "sx" | "sy"> {
  const squash = impact * 0.26 * Math.exp(-ms / 70) * Math.cos(ms / 38);
  return { sx: 1 + squash * 0.8, sy: 1 - squash };
}

function landFrame(p: MovePlan, ms: number): MoveFrame {
  let start = 0;
  let impact = p.impact;
  for (const b of p.bounces) {
    if (ms < start + b.ms) {
      const v = (ms - start) / b.ms;
      const at = { x: p.to.x, y: p.to.y - b.height * 4 * v * (1 - v) };
      return frame(at, p.to.size, "land", landSquash(ms - start, impact));
    }
    start += b.ms;
    impact *= 0.45;
  }
  return frame(p.to, p.to.size, "land", landSquash(ms - start, impact));
}

/** Where the pet is `ms` into a move, and how it looks. */
export function sampleMove(p: MovePlan, ms: number): MoveFrame {
  if (ms < p.windup) return windupFrame(p, ms / p.windup);
  const t = ms - p.windup;
  if (t < p.travel) {
    if (p.kind === "walk") return walkFrame(p, t / p.travel);
    if (p.kind === "drop") return fallFrame(p, t);
    return flightFrame(p, t / p.travel);
  }
  const l = t - p.travel;
  if (l < p.land) return landFrame(p, l);
  return frame(p.to, p.to.size, "done");
}

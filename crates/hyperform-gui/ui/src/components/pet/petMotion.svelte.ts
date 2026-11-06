import { drawnFeet, grab, letGo, pull, stepHeld, type Held } from "$lib/domain/pet/drag";
import {
  FLING_SPEED,
  TURN_MS,
  flingAim,
  planDrop,
  planFling,
  planHop,
  planMove,
  retargetMove,
  sampleMove,
  type Facing,
  type MoveFrame,
  type MoveKind,
  type MovePlan,
} from "$lib/domain/pet/motion";
import { PET_FEET, type PetSpot } from "$lib/domain/pet/perches";
import type { Point } from "$lib/domain/pet/physics";

export type MotionPhase = "rest" | "drag" | MoveKind;

/** How quickly a move blends out of the pose it started in. */
const CARRY_MS = 90;
const SAME_SPOT = 0.75;

function same(a: PetSpot | null, b: PetSpot): boolean {
  return !!a && Math.abs(a.x - b.x) < SAME_SPOT && Math.abs(a.y - b.y) < SAME_SPOT && Math.abs(a.size - b.size) < 0.5;
}

/**
 * The roaming pet's body in motion: one animation-frame loop, running only
 * while something moves, plays the planner's hops, walks, flings and drops
 * (`lib/domain/pet/motion.ts`), and while it is held follows the pointer
 * and swings from your grip (`drag.ts`). Everything it outputs is a
 * transform: where its feet are, its squash, lean and facing. Under Reduce
 * Motion every move is a jump.
 */
export class PetMotion {
  /** Its feet, in viewport pixels, and its size now. */
  x = $state(0);
  y = $state(0);
  size = $state(28);
  sx = $state(1);
  sy = $state(1);
  rot = $state(0);
  /** Horizontal facing, -1..1: it passes through 0 as it turns around. */
  face = $state(1);
  /** Feet lift and body bob, in the pet's own 32-unit space. */
  lift = $state<[number, number]>([0, 0]);
  bob = $state(0);
  phase = $state<MotionPhase>("rest");
  airborne = $state(false);
  /** Where it is held, from its box's top-left, while dragged. */
  grip = $state<Point | null>(null);

  #reduced: boolean;
  #frame = 0;
  #ticking = false;
  #last = 0;
  #plan: MovePlan | null = null;
  #planAt = 0;
  #carry = { rot: 0, sx: 0, sy: 0 };
  #target: PetSpot | null = null;
  #pending: { spot: PetSpot; sameEdge: boolean } | null = null;
  #landed: ((dizzy: boolean) => void) | null = null;
  #dizzy = false;
  #facing: Facing = 1;
  #turn = { from: 1, to: 1, at: -Infinity };
  #held: Held | null = null;
  #bounds = { width: 1200, height: 800 };

  constructor(reduced: boolean) {
    this.#reduced = reduced;
  }

  get resting(): boolean {
    return this.phase === "rest";
  }

  /**
   * The rig's transform for a pet drawn `drawn` px wide: lean, squash and
   * its size now. Facing is separate (`face`), mirrored inside the pet so
   * it turns about its own middle wherever it is held.
   */
  rig(drawn: number): string {
    const k = this.size / Math.max(1, drawn);
    return `rotate(${this.rot.toFixed(2)}deg) scale(${(this.sx * k).toFixed(4)}, ${(this.sy * k).toFixed(4)})`;
  }

  setBounds(width: number, height: number) {
    this.#bounds = { width, height };
  }

  /** Straight onto `spot`, with no move. */
  place(spot: PetSpot) {
    this.#plan = null;
    this.#pending = null;
    this.#target = spot;
    this.#apply({ x: spot.x, y: spot.y, size: spot.size, sx: 1, sy: 1, rot: 0, lift: [0, 0], bob: 0 });
    this.phase = "rest";
    this.airborne = false;
  }

  /** Its perch moved under it (a scroll, a resize): it stays on the same spot, and a move under way carries on. */
  ride(spot: PetSpot) {
    if (this.#held || same(this.#heading(), spot)) return;
    if (this.resting || this.#reduced) this.place(spot);
    else if (this.#pending) this.#pending = { spot, sameEdge: true };
    else if (this.#plan) {
      this.#plan = retargetMove(this.#plan, spot);
      this.#target = spot;
    }
  }

  /** Walks along its edge, or hops to another perch; a move in the air finishes first. */
  goTo(spot: PetSpot, sameEdge: boolean) {
    if (this.#held || same(this.#heading(), spot)) return;
    if (this.#reduced) return this.place(spot);
    if (this.#plan && this.#plan.kind !== "walk") {
      this.#pending = { spot, sameEdge };
      return;
    }
    const plan = planMove(this.#here(), spot, { sameEdge, facing: this.#facing, top: spot.size });
    if (plan) this.#start(plan);
    else this.place(spot);
  }

  /** A hop on the spot, for joy or a trick. */
  hop(height = this.size * 0.5) {
    if (this.#reduced || !this.resting || !this.#target) return;
    this.#start(planHop(this.#here(), height, this.#facing));
  }

  /** Picked up at the pointer (viewport px). */
  grab(px: number, py: number) {
    this.#plan = null;
    this.#pending = null;
    this.#landed = null;
    this.#held = grab({ x: this.x, y: this.y }, { x: px, y: py }, performance.now());
    this.grip = { x: this.size / 2 + px - this.x, y: this.size * PET_FEET + py - this.y };
    this.phase = "drag";
    this.airborne = false;
    this.#wake();
  }

  dragTo(px: number, py: number) {
    if (this.#held) this.#held = pull(this.#held, { x: px, y: py }, performance.now());
  }

  /**
   * Let go. Thrown, it keeps its speed and curves onto the spot `choose`
   * picks where it is heading; let go gently, it drops onto the nearest
   * one. `landed` runs as it touches down.
   */
  release(choose: (aim: Point) => PetSpot | null, landed?: (dizzy: boolean) => void) {
    const held = this.#held;
    if (!held) return;
    this.#held = null;
    const feet = drawnFeet({ x: this.x, y: this.y }, held.offset, this.rot, this.sx, this.sy);
    this.x = feet.x;
    this.y = feet.y;
    this.grip = null;
    const here = this.#here();
    const thrown = letGo(held, performance.now());
    const flung = !this.#reduced && thrown.speed > FLING_SPEED;
    const spot = choose(flung ? flingAim(here, thrown.v) : here);
    if (!spot || this.#reduced) {
      this.place(spot ?? here);
      landed?.(false);
      return;
    }
    this.#dizzy = thrown.dizzy;
    this.#landed = landed ?? null;
    this.#start(flung ? planFling(here, thrown.v, spot, this.#bounds) : planDrop(here, spot, this.#facing));
  }

  destroy() {
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
  }

  #heading(): PetSpot | null {
    return this.#pending?.spot ?? this.#target;
  }

  #here(): PetSpot {
    return { x: this.x, y: this.y, size: this.size };
  }

  #start(plan: MovePlan) {
    const first = sampleMove(plan, 0);
    this.#carry = { rot: this.rot - first.rot, sx: this.sx - first.sx, sy: this.sy - first.sy };
    this.#plan = plan;
    this.#planAt = performance.now();
    this.#target = plan.to;
    this.phase = plan.kind;
    if (plan.facing !== this.#facing) this.#turnTo(plan.facing);
    this.#wake();
  }

  #turnTo(facing: Facing) {
    this.#turn = { from: this.face, to: facing, at: performance.now() };
    this.#facing = facing;
    this.#wake();
  }

  /** Starts the loop unless it is running; inside a frame, the frame schedules the next one. */
  #wake() {
    if (this.#frame || this.#ticking) return;
    this.#last = performance.now();
    this.#frame = requestAnimationFrame(this.#tick);
  }

  #tick = (now: number) => {
    this.#frame = 0;
    this.#ticking = true;
    const dt = Math.min(34, Math.max(0, now - this.#last));
    this.#last = now;
    if (this.#held) this.#stepHeld(this.#held, dt, now);
    else if (this.#plan) this.#stepPlan(this.#plan, now);
    const turning = this.#stepFace(now);
    this.#ticking = false;
    if (turning || this.#plan || this.#held) this.#frame = requestAnimationFrame(this.#tick);
  };

  #stepPlan(plan: MovePlan, now: number) {
    const t = now - this.#planAt;
    const f = sampleMove(plan, t);
    const fade = Math.exp(-t / CARRY_MS);
    const { rot, sx, sy } = this.#carry;
    this.#apply({ ...f, rot: f.rot + rot * fade, sx: f.sx + sx * fade, sy: f.sy + sy * fade });
    this.airborne = f.phase === "air";
    if (f.phase !== "done") return;
    this.#plan = null;
    this.phase = "rest";
    const landed = this.#landed;
    this.#landed = null;
    landed?.(this.#dizzy);
    this.#dizzy = false;
    const next = this.#pending;
    this.#pending = null;
    if (next) this.goTo(next.spot, next.sameEdge);
  }

  #stepHeld(held: Held, dt: number, now: number) {
    if (this.#reduced) {
      this.#apply({ ...held.aim, size: this.size, sx: 1, sy: 1, rot: 0, lift: [0, 0], bob: 0 });
      return;
    }
    const step = stepHeld(held, dt, now, this.size);
    this.#held = step.held;
    const { facing, ...frame } = step.frame;
    if (facing && facing !== this.#facing) this.#turnTo(facing);
    this.#apply({ ...frame, size: this.size });
  }

  #stepFace(now: number): boolean {
    const u = this.#reduced ? 1 : Math.min(1, (now - this.#turn.at) / TURN_MS);
    this.face = this.#turn.from + (this.#turn.to - this.#turn.from) * (0.5 - 0.5 * Math.cos(Math.PI * u));
    return u < 1;
  }

  #apply(f: Omit<MoveFrame, "phase">) {
    this.x = f.x;
    this.y = f.y;
    this.size = f.size;
    this.sx = f.sx;
    this.sy = f.sy;
    this.rot = f.rot;
    this.lift = f.lift;
    this.bob = f.bob;
  }
}

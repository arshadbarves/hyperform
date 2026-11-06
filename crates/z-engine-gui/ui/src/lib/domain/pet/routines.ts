import type { PetTrick } from "./looks";
import type { Gaze } from "./pose";
import type { Point } from "./physics";

/**
 * The pet's idle life, as pure choices: which routine it plays while it
 * sits on an edge, how it blinks, and where its eyes point.
 */

/** Something the pet does while idle: the tricks it has unlocked, and everyday routines. */
export type PetRoutine = PetTrick | "lookAround" | "yawn" | "sit" | "lieDown" | "wave" | "tap";

/** How long each routine plays. */
export const ROUTINE_MS: Record<PetRoutine, number> = {
  stretch: 1400,
  hop: 900,
  spin: 1000,
  sparkle: 1400,
  lookAround: 2600,
  yawn: 1900,
  sit: 9000,
  lieDown: 14_000,
  wave: 1600,
  tap: 1800,
};

/** Idle this long and the pet yawns and lies down more than it plays. */
export const DROWSY_MS = 60_000;

type Weighted = [PetRoutine, number][];

function weights(tricks: readonly PetTrick[], drowsy: boolean): Weighted {
  if (drowsy) {
    const out: Weighted = [
      ["yawn", 3],
      ["lieDown", 3],
      ["sit", 2],
      ["lookAround", 1],
    ];
    if (tricks.includes("stretch")) out.push(["stretch", 1]);
    return out;
  }
  const out: Weighted = [
    ["lookAround", 3],
    ["sit", 2],
    ["tap", 1],
    ["yawn", 1],
  ];
  for (const trick of tricks) out.push([trick, 3 / tricks.length]);
  return out;
}

/** The routine for an idle moment; `random` in [0, 1). */
export function pickRoutine(input: { tricks: readonly PetTrick[]; idleMs: number; random: number }): PetRoutine {
  const table = weights(input.tricks, input.idleMs >= DROWSY_MS);
  const total = table.reduce((sum, [, w]) => sum + w, 0);
  let left = Math.min(Math.max(input.random, 0), 0.999_999) * total;
  for (const [routine, w] of table) {
    if (left < w) return routine;
    left -= w;
  }
  return table[table.length - 1][0];
}

/** One blink: how long the eyes stay shut, then open, then shut… (ms). Mostly single, sometimes double or slow. */
export function blinkPattern(random: number): number[] {
  if (random < 0.65) return [120];
  if (random < 0.85) return [100, 90, 110];
  return [260];
}

/** The wait before the next blink. */
export function blinkGap(random: number): number {
  return 2200 + random * 4200;
}

/** Eye offsets for each fixed gaze, in the pet's 32-unit space. */
export const GAZE_OFFSET: Record<Exclude<Gaze, "pointer" | "scan">, Point> = {
  center: { x: 0, y: 0 },
  down: { x: 0, y: 1.5 },
  up: { x: 0, y: -1.7 },
  left: { x: -2, y: 0.3 },
  right: { x: 2, y: 0.3 },
};

/**
 * The eyes' offset toward `target` from the pet's middle `center` (both
 * viewport pixels) for a pet drawn `size` px wide. They reach their edge a
 * little beyond the pet; `facing` -1 undoes a mirrored body.
 */
export function gazeToward(center: Point, target: Point, size: number, facing: 1 | -1 = 1): Point {
  const dx = target.x - center.x;
  const dy = target.y - center.y;
  const dist = Math.hypot(dx, dy) || 1;
  const reach = Math.min(1, dist / Math.max(12, size * 1.3));
  return { x: (dx / dist) * 2 * reach * facing, y: (dy / dist) * 1.8 * reach };
}

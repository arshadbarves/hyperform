import { Spring } from "svelte/motion";
import type { Point } from "$lib/domain/pet/physics";
import type { Gaze } from "$lib/domain/pet/pose";
import { GAZE_OFFSET, blinkGap, blinkPattern, gazeToward } from "$lib/domain/pet/routines";
import { SPRING, prefersReducedMotion } from "$lib/ui/motion";

interface EyesInput {
  root: () => Element | undefined;
  gaze: () => Gaze;
  /** A point to look at (the caret, the pointer), overriding the gaze. */
  lookAt: () => Point | null;
  facing: () => 1 | -1;
  /** The eyes are open ellipses that can blink and follow. */
  open: () => boolean;
}

/**
 * The pet's eyes: where they look (a spring toward the pose's gaze, the
 * pointer while hovered, or `lookAt`) and when they blink (mostly once,
 * sometimes twice or slowly). No blinking and no easing under Reduce
 * Motion. Call during component init.
 */
export function usePetEyes(input: EyesInput) {
  const reduced = prefersReducedMotion();
  const offset = new Spring({ x: 0, y: 0 }, SPRING.snappy);
  let blinking = $state(false);
  let pointer = $state<Point | null>(null);

  $effect(() => {
    if (input.gaze() !== "pointer" || input.lookAt()) return;
    const onMove = (e: PointerEvent) => (pointer = { x: e.clientX, y: e.clientY });
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      pointer = null;
    };
  });

  $effect(() => {
    const gaze = input.gaze();
    const target = input.open() ? (input.lookAt() ?? (gaze === "pointer" ? pointer : null)) : null;
    const el = input.root();
    let next: Point = gaze === "pointer" || gaze === "scan" ? { x: 0, y: 0 } : GAZE_OFFSET[gaze];
    if (target && el) {
      const box = el.getBoundingClientRect();
      next = gazeToward({ x: box.left + box.width / 2, y: box.top + box.height / 2 }, target, box.width, input.facing());
    }
    void offset.set(next, { instant: reduced });
  });

  $effect(() => {
    if (reduced || !input.open()) return;
    let timer = 0;
    const play = (steps: number[], i: number) => {
      if (i >= steps.length) {
        blinking = false;
        schedule();
        return;
      }
      blinking = i % 2 === 0;
      timer = window.setTimeout(() => play(steps, i + 1), steps[i]);
    };
    const schedule = () => {
      timer = window.setTimeout(() => play(blinkPattern(Math.random()), 0), blinkGap(Math.random()));
    };
    schedule();
    return () => {
      window.clearTimeout(timer);
      blinking = false;
    };
  });

  return {
    get x(): number {
      return offset.current.x;
    },
    get y(): number {
      return offset.current.y;
    },
    get blinking(): boolean {
      return blinking;
    },
  };
}

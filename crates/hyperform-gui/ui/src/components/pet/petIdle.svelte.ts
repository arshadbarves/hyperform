import { untrack } from "svelte";
import { pickWander, type Behavior } from "$lib/domain/pet/behavior";
import type { PerchId } from "$lib/domain/pet/perches";
import { ROUTINE_MS, pickRoutine, type PetRoutine } from "$lib/domain/pet/routines";
import { pet } from "$lib/runtime";
import { petUi } from "$lib/stores/pet.svelte";
import { userSignals } from "$lib/stores/userSignals.svelte";

/** Where the pet likes to stand on each edge, as a share of its length (0..1); it keeps clear of Send and window buttons. */
const REACH: Partial<Record<PerchId, [number, number]>> = {
  composer: [0.02, 0.3],
  sidebar: [0.1, 0.9],
  panel: [0.05, 0.4],
};

function within(perch: PerchId, random: number): number {
  const [lo, hi] = REACH[perch] ?? [0.5, 0.5];
  return lo + (hi - lo) * random;
}

interface IdleInput {
  behavior: () => Behavior | null;
  available: () => ReadonlySet<PerchId>;
  /** Since the user or the agent last did anything. */
  idleMs: () => number;
  reduced: boolean;
  /** A hop on the spot (the motion engine's). */
  hop: () => void;
}

/**
 * The idle life of a roaming pet: now and then it strolls along its edge,
 * wanders to another one, or plays a routine (looks around, yawns, sits or
 * lies down, taps its foot, or a trick it has unlocked); it hops hello when
 * you come back. Timers run only while it sits somewhere other than the
 * island, and never under Reduce Motion. Call during component init.
 */
export function usePetIdle(input: IdleInput) {
  const alongs = $state<Partial<Record<PerchId, number>>>({ composer: 0.08, sidebar: 0.5, panel: 0.2 });
  const resting = $derived.by(() => {
    const b = input.behavior();
    return b && b.activity === "sit" && b.perch !== "island" ? b.perch : null;
  });

  $effect(() => {
    const perch = resting;
    if (!perch || input.reduced) return;
    const timers = new Set<number>();
    const later = (ms: number, run: () => void) => {
      const id = window.setTimeout(() => {
        timers.delete(id);
        run();
      }, ms);
      timers.add(id);
    };
    const every = (min: number, spread: number, run: () => void) =>
      later(min + Math.random() * spread, () => {
        run();
        every(min, spread, run);
      });
    const play = (routine: PetRoutine) => {
      petUi.routine = routine;
      if (routine === "hop") input.hop();
      later(ROUTINE_MS[routine], () => (petUi.routine = null));
    };
    every(12_000, 9_000, () => {
      if (!petUi.routine) alongs[perch] = within(perch, Math.random());
    });
    every(32_000, 30_000, () => {
      const next = petUi.routine ? null : pickWander(input.available(), perch, Math.random());
      if (!next) return;
      alongs[next] = within(next, Math.random());
      petUi.wander = next;
    });
    every(8_000, 14_000, () => {
      if (!petUi.routine) play(pickRoutine({ tricks: pet.tricks, idleMs: input.idleMs(), random: Math.random() }));
    });
    return () => {
      timers.forEach((t) => window.clearTimeout(t));
      petUi.routine = null;
    };
  });

  $effect(() => {
    if (userSignals.returnedAt === null || input.reduced) return;
    untrack(() => {
      if (resting) input.hop();
    });
  });

  return {
    along(perch: PerchId): number {
      return alongs[perch] ?? 0.5;
    },
    place(perch: PerchId, along: number) {
      alongs[perch] = along;
    },
  };
}

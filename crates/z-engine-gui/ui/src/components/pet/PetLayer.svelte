<script lang="ts">
  import { untrack } from "svelte";
  import { petBehavior, reactionPose, type PetActivity } from "$lib/domain/pet/behavior";
  import { alongFor, nearestPerch, PERCH_SIZE, PET_FEET, spotOn, STAGE_PERCHES, usable, type PerchId } from "$lib/domain/pet/perches";
  import { TYPING_MS } from "$lib/domain/pet/pose";
  import { pet } from "$lib/runtime";
  import { island } from "$lib/stores/island.svelte";
  import type { Live } from "$lib/stores/live.svelte";
  import { petUi } from "$lib/stores/pet.svelte";
  import { currentStage } from "$lib/stores/stage.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import { perches } from "$lib/ui/perch.svelte";
  import Pet from "./Pet.svelte";
  import PetCardPopover from "./PetCardPopover.svelte";
  import { useLookTarget } from "./petGaze.svelte";
  import { usePetIdle } from "./petIdle.svelte";
  import { PetMotion } from "./petMotion.svelte";

  /**
   * The roaming pet, on a click-through layer above the app and below every
   * popover. `petBehavior` says where it should be; the motion engine walks
   * it along an edge or hops it to another perch, and the island's own slot
   * takes over whenever it is home. Drag it anywhere and let go: thrown, it
   * flies onto the perch it is heading for; dropped, it falls onto the
   * nearest one. Click to boop; double-click for its card. Only transforms
   * change as it moves.
   */
  type Props = { live: Live };
  let { live }: Props = $props();

  const reduced = prefersReducedMotion();
  const motion = new PetMotion(reduced);
  let viewport = $state({ width: 1200, height: 800 });
  let menusOpen = $state(false);
  let lastWorkAt = $state(Date.now());
  let actor: HTMLDivElement | undefined = $state();
  let press: { x: number; y: number; moved: boolean } | null = null;
  /** The perch and place it was last sent to, to tell a scroll from a move. */
  let sent: { perch: PerchId; along: number } | null = null;

  const rects = $derived.by(() => {
    // The expanded side panel covers the stage and the perches on it.
    const covered = ui.panel.open && ui.panel.expanded;
    return perches.rects().filter((r) => usable(r, viewport) && !(covered && STAGE_PERCHES.has(r.id)));
  });
  const available = $derived(new Set(rects.map((r) => r.id)));
  const typing = $derived(userSignals.typingAt !== null && live.now - userSignals.typingAt < TYPING_MS);
  const idleMs = $derived(live.now - Math.max(userSignals.lastActiveAt, lastWorkAt));
  const behavior = $derived(
    live.pose
      ? petBehavior({
          level: live.level,
          roam: petUi.roam,
          status: live.status,
          stage: currentStage(),
          typing,
          dragging: petUi.dragging,
          overlayOpen: island.open || island.contextOpen || ui.paletteOpen || ui.worktreeOpen || menusOpen,
          idleMs,
          available,
          current: petUi.perch,
          wander: petUi.wander,
        })
      : null,
  );
  const drawn = $derived(behavior?.size ?? PERCH_SIZE.island);
  const hopHeight = (share: number) => motion.hop(motion.size * share);
  const idle = usePetIdle({
    behavior: () => behavior,
    available: () => available,
    idleMs: () => idleMs,
    reduced,
    hop: () => hopHeight(0.45),
  });
  const quiet = $derived(live.status.kind === "idle");
  const look = useLookTarget({ pointer: () => quiet, enabled: () => live.level === "lively" && !reduced });
  const flying = $derived(motion.airborne && motion.phase !== "hop");
  const activity = $derived<PetActivity>(
    flying ? "drag" : motion.phase === "walk" ? "walk" : (behavior?.activity ?? "sit"),
  );
  const face = $derived(
    live.pose && behavior ? reactionPose(live.pose, activity, petUi.reactionAt(live.now), petUi.routine) : null,
  );
  const celebrating = $derived(behavior?.activity === "celebrate");

  $effect(() => {
    if (live.status.kind === "working") lastWorkAt = live.now;
  });

  // A level-up celebrates once nothing needs you; calm and off skip the show.
  $effect(() => {
    if (!pet.levelUp || live.status.kind === "attention") return;
    untrack(() => {
      if (live.level === "lively") {
        petUi.react("levelUp");
        hopHeight(0.7);
      }
      pet.celebrated();
    });
  });

  // A helper's changes applied: a happy hop (the count's first reading is the baseline).
  let applied = -1;
  $effect(() => {
    const count = pet.growth.applied;
    if (!pet.loaded) return;
    untrack(() => {
      if (applied >= 0 && count > applied && live.level === "lively") {
        petUi.react("applied");
        hopHeight(0.5);
      }
      applied = count;
    });
  });

  $effect(() => {
    if (celebrating) untrack(() => hopHeight(0.55));
  });

  // Perches can move without resizing (a panel sliding in), so check now and then.
  $effect(() => {
    void live.now;
    perches.measure();
    menusOpen = document.querySelector(".menu, .chip-pop, .composer-pop") !== null;
  });

  $effect(() => {
    const measure = () => {
      const { innerWidth: width, innerHeight: height } = window;
      viewport = { width, height };
      motion.setBounds(width, height);
      perches.measure();
    };
    const onScroll = () => perches.measure();
    measure();
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", onScroll, { capture: true, passive: true });
    return () => {
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", onScroll, { capture: true });
      motion.destroy();
    };
  });

  // Go where the behavior says: stay put as its perch scrolls, walk along
  // the same edge, hop to another perch; leave the island from its slot.
  $effect(() => {
    const b = behavior;
    if (!b) {
      untrack(() => (petUi.docked = true));
      return;
    }
    const rect = rects.find((r) => r.id === b.perch);
    if (!rect) return;
    const along = idle.along(b.perch);
    const spot = spotOn(rect, along, b.size);
    untrack(() => {
      if (petUi.dragging) return;
      if (b.perch !== "island" && petUi.docked) {
        const home = rects.find((r) => r.id === "island");
        motion.place(home ? spotOn(home, 0.5, PERCH_SIZE.island) : spot);
        petUi.docked = false;
        sent = null;
      }
      petUi.perch = b.perch;
      if (petUi.docked || (sent?.perch === b.perch && sent.along === along)) motion.ride(spot);
      else motion.goTo(spot, sent?.perch === b.perch && rect.kind === "edge");
      sent = { perch: b.perch, along };
    });
  });

  // Where it is, for the island's portrait to look at while it roams.
  $effect(() => {
    const at = petUi.docked || !face ? null : { x: motion.x, y: motion.y - drawn * (PET_FEET - 0.5) };
    untrack(() => {
      petUi.at = at;
    });
  });
  $effect(() => () => (petUi.at = null));

  // Home: once it has landed in the island's slot, the island shows it.
  $effect(() => {
    if (behavior?.perch !== "island" || petUi.docked || petUi.dragging || !motion.resting) return;
    const home = rects.find((r) => r.id === "island");
    if (!home) return;
    const spot = spotOn(home, 0.5, PERCH_SIZE.island);
    if (Math.hypot(motion.x - spot.x, motion.y - spot.y) < 1.5 && Math.abs(motion.size - PERCH_SIZE.island) < 0.5) {
      petUi.docked = true;
      if (petUi.wander === "island") petUi.wander = null;
    }
  });

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !actor) return;
    actor.setPointerCapture(e.pointerId);
    press = { x: e.clientX, y: e.clientY, moved: false };
  }

  function onPointerMove(e: PointerEvent) {
    if (!press) return;
    if (!press.moved) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < 4) return;
      press.moved = true;
      petUi.dragging = true;
      motion.grab(press.x, press.y);
    }
    motion.dragTo(e.clientX, e.clientY);
  }

  function onPointerUp(e: PointerEvent) {
    const p = press;
    press = null;
    if (!p) return;
    if (!p.moved) {
      // The second click of a double-click opens the card instead.
      if (e.detail >= 2) return;
      petUi.boop();
      hopHeight(0.22);
      return;
    }
    motion.release(
      (aim) => {
        const target = nearestPerch(rects, aim.x, aim.y - motion.size / 2);
        if (!target) return null;
        const along = alongFor(target, aim.x);
        idle.place(target.id, along);
        petUi.wander = target.id;
        sent = { perch: target.id, along };
        return spotOn(target, along);
      },
      (dizzy) => {
        if (dizzy && live.level === "lively") petUi.react("dizzy");
      },
    );
    petUi.dragging = false;
  }
</script>

{#if face && behavior && !petUi.docked}
  <div class="pet-layer" aria-hidden="true">
    <div
      bind:this={actor}
      class={`pet-actor activity-${activity}`}
      class:is-dragging={petUi.dragging}
      class:is-flying={flying}
      style:transform={`translate3d(${(motion.x - drawn / 2).toFixed(2)}px, ${(motion.y - drawn * PET_FEET).toFixed(2)}px, 0)`}
      style:--pet-size={`${drawn}px`}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
      ondblclick={() => petUi.openCard()}
      oncontextmenu={(e) => {
        e.preventDefault();
        petUi.openCard();
      }}
      role="presentation"
    >
      <div
        class="pet-rig-wrap"
        style:transform={motion.rig(drawn)}
        style:transform-origin={motion.grip ? `${motion.grip.x.toFixed(1)}px ${motion.grip.y.toFixed(1)}px` : null}
        style:--pet-face={motion.face.toFixed(3)}
        style:--pet-lf={motion.lift[0].toFixed(2)}
        style:--pet-rf={motion.lift[1].toFixed(2)}
        style:--pet-bob={motion.bob.toFixed(2)}
      >
        <Pet
          pose={face}
          look={petUi.look}
          stage={pet.stage}
          wearing={pet.growth.wearing}
          helpers={live.status.helpers.running}
          size={drawn}
          facing={motion.face < 0 ? -1 : 1}
          routine={petUi.routine}
          lookAt={look.point}
          turn={motion.face}
          lift={motion.lift}
          bob={motion.bob}
        />
      </div>
    </div>
  </div>
{/if}

<PetCardPopover anchor={petUi.docked ? perches.node("island") : (actor ?? null)} below={petUi.docked} />

<script lang="ts">
  import { canBlink, expression } from "$lib/domain/pet/emotions";
  import type { PetAccessory as Accessory, PetLook, PetStage } from "$lib/domain/pet/looks";
  import type { Point } from "$lib/domain/pet/physics";
  import type { PetPose } from "$lib/domain/pet/pose";
  import type { PetRoutine } from "$lib/domain/pet/routines";
  import PetAccessory from "./PetAccessory.svelte";
  import PetBubble from "./PetBubble.svelte";
  import PetFace from "./PetFace.svelte";
  import PetProps from "./PetProps.svelte";
  import PetRing from "./PetRing.svelte";
  import { usePetEyes } from "./petEyes.svelte";

  /**
   * The flat pet, in SVG: a soft pearl creature whose face, posture, props
   * and bubbles show the agent's mood (`lib/domain/pet/emotions.ts`). Its
   * look tints the body; the status tone tints only the aura. Purely
   * decorative: the status line carries the meaning. Where it stands and
   * how it moves belong to whoever places it (the roaming layer's motion
   * engine). `Pet` draws it where the 3D pet cannot be drawn.
   */
  type Props = {
    pose: PetPose;
    look?: PetLook;
    stage?: PetStage;
    wearing?: Accessory | null;
    /** Plan progress drawn as a ring around the pet, 0..1. */
    progress?: number | null;
    /** Running agents and jobs, drawn as orbiting sprites (at most four). */
    helpers?: number;
    size?: number;
    /** Which way it faces, so its eyes follow `lookAt` the right way round. */
    facing?: 1 | -1;
    /** An idle routine or trick it is playing. */
    routine?: PetRoutine | null;
    /** A point on screen to look at (the caret, the pointer). */
    lookAt?: Point | null;
  };

  let {
    pose,
    look = "pearl",
    stage = "sprout",
    wearing = null,
    progress = null,
    helpers = 0,
    size = 28,
    facing = 1,
    routine = null,
    lookAt = null,
  }: Props = $props();

  const uid = $props.id();
  const BODY = "M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z";

  let root: SVGSVGElement | undefined = $state();
  const mood = $derived(expression(pose));
  const gaze = $derived(pose.gaze);
  const open = $derived(canBlink(mood.face.eyes));
  const eyes = usePetEyes({
    root: () => root,
    gaze: () => gaze,
    lookAt: () => (gaze === "scan" || gaze === "up" ? null : lookAt),
    facing: () => facing,
    open: () => open,
  });
  const sprites = $derived(Array.from({ length: Math.min(4, helpers) }, (_, i) => i));
  const classes = $derived(
    [
      "pet",
      `mood-${pose.mood}`,
      `body-${mood.body}`,
      `tone-${pose.tone}`,
      `gaze-${gaze}`,
      `look-${look}`,
      `stage-${stage}`,
      routine && `routine-${routine}`,
    ]
      .filter(Boolean)
      .join(" "),
  );
</script>

<span class={`pet-breath breath-${mood.body}`}>
  <svg
    bind:this={root}
    class={classes}
    width={size}
    height={size}
    viewBox="0 0 32 32"
    aria-hidden="true"
    style:--gx={eyes.x}
    style:--gy={eyes.y}
  >
    <defs>
      <radialGradient id={`${uid}-body`} cx="36%" cy="28%" r="82%">
        <stop offset="0%" class="pet-stop-light" />
        <stop offset="52%" class="pet-stop-mid" />
        <stop offset="100%" class="pet-stop-edge" />
      </radialGradient>
      <radialGradient id={`${uid}-bloom`} cx="50%" cy="62%" r="48%">
        <stop offset="0%" class="pet-bloom-core" />
        <stop offset="100%" class="pet-bloom-fade" />
      </radialGradient>
      <filter id={`${uid}-glow`} x="-60%" y="-60%" width="220%" height="220%">
        <feGaussianBlur stdDeviation="2.6" />
      </filter>
    </defs>

    {#if progress !== null}<PetRing {progress} />{/if}

    {#each sprites as i (i)}
      <circle class="pet-sprite" cx="16" cy="2.6" r="1.6" style:--i={i} />
    {/each}

    <g class="pet-turn">
      <g class="pet-rig">
        <g class="pet-stride">
          <g class="pet-body">
            <path class="pet-aura" d={BODY} filter={`url(#${uid}-glow)`} />
            <ellipse class="pet-foot is-left" cx="11.8" cy="27.2" rx="2.6" ry="1.5" />
            <ellipse class="pet-foot is-right" cx="20.2" cy="27.2" rx="2.6" ry="1.5" />
            <path d={BODY} fill={`url(#${uid}-body)`} />
            <path class="pet-bloom" d={BODY} fill={`url(#${uid}-bloom)`} />
            <path class="pet-rim" d={BODY} />
            <ellipse class="pet-shine" cx="11.8" cy="12.9" rx="3.4" ry="1.9" transform="rotate(-28 11.8 12.9)" />
            <PetFace face={mood.face} blinking={eyes.blinking} />
            {#if wearing}<PetAccessory kind={wearing} />{/if}
            <PetProps prop={mood.prop} />
          </g>
        </g>
      </g>
    </g>

    <PetBubble bubble={mood.bubble} fx={mood.fx} />
  </svg>
</span>

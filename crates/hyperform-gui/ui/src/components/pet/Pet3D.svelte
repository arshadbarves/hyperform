<script lang="ts">
  import { onDestroy } from "svelte";
  import { canBlink, expression } from "$lib/domain/pet/emotions";
  import { PetScene, environment, petFrames, renderView, webgl, type PetSceneState } from "$lib/pet3d";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import PetBubble from "./PetBubble.svelte";
  import PetRing from "./PetRing.svelte";
  import { usePetEyes } from "./petEyes.svelte";
  import type { PetViewProps } from "./petView";

  /**
   * The 3D pet (lib/pet3d): its scene drawn into a canvas half as big again
   * as its box, so what it holds and wears is not clipped, over the flat
   * pet's progress ring and under its bubbles and bursts, in the same
   * 32-unit space. Breathing stays the CSS loop on the wrapper. The shared
   * frame loop draws it only while something plays, once per change under
   * Reduce Motion, and not at all while it just breathes.
   */
  let {
    pose,
    look = "pearl",
    stage = "sprout",
    wearing = null,
    progress = null,
    helpers = 0,
    size = 28,
    routine = null,
    lookAt = null,
    turn = 0,
    lift = [0, 0],
    bob = 0,
    framing = "full",
    head = null,
  }: PetViewProps = $props();

  const reduced = prefersReducedMotion();
  const scene = new PetScene();
  let root: HTMLSpanElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();

  const mood = $derived(expression(pose));
  const gaze = $derived(pose.gaze);
  const open = $derived(canBlink(mood.face.eyes));
  // It turns to face its way rather than mirroring, so the gaze needs no flip.
  const eyes = usePetEyes({
    root: () => root,
    gaze: () => gaze,
    lookAt: () => (gaze === "scan" || gaze === "up" ? null : lookAt),
    facing: () => 1,
    open: () => open,
  });
  const portrait = $derived(framing === "portrait");
  /** The canvas's CSS size; the portrait fills its box. */
  const view = $derived(portrait ? size : size * 1.5);
  const shown = $derived<PetSceneState>({
    face: mood.face,
    body: mood.body,
    prop: mood.prop,
    mood: pose.mood,
    tone: pose.tone,
    look,
    stage,
    wearing,
    routine,
    helpers: portrait ? 0 : helpers,
    gaze: { x: eyes.x, y: eyes.y },
    scan: gaze === "scan",
    searching: pose.mood === "searching",
    blinking: eyes.blinking,
    turn,
    lift: [lift[0], lift[1]],
    bob,
    head,
    still: reduced,
  });
  const classes = $derived(
    ["pet", "pet-3d", `mood-${pose.mood}`, `tone-${pose.tone}`, `look-${look}`, `stage-${stage}`].join(" "),
  );

  const frames = petFrames.add({
    draw: (now) => {
      if (!canvas) return;
      scene.update(shown, now);
      renderView(canvas, scene.scene, scene.camera, view, view);
    },
    rate: (drawnAt) => scene.rate(drawnAt),
  });

  // The pose object is rebuilt as the status ticks; draw only when what it shows changes.
  let drawn = "";
  $effect(() => {
    const key = JSON.stringify([shown, view, !!canvas]);
    if (key === drawn) return;
    drawn = key;
    frames.invalidate();
  });
  $effect(() => {
    scene.setFraming(framing);
    frames.invalidate();
  });
  // A restored context has a new reflection map.
  $effect(() => {
    void webgl.restores;
    scene.setEnvironment(environment());
    frames.invalidate();
  });
  onDestroy(() => {
    frames.remove();
    scene.dispose();
  });
</script>

<span class={`pet-breath breath-${mood.body}`}>
  <span bind:this={root} class={classes} style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
    {#if progress !== null && !portrait}
      <svg class="pet-3d-layer" viewBox="0 0 32 32"><PetRing {progress} /></svg>
    {/if}
    <canvas bind:this={canvas} class="pet-3d-canvas" class:is-portrait={portrait}></canvas>
    {#if !portrait}
      <svg class="pet-3d-layer" viewBox="0 0 32 32"><PetBubble bubble={mood.bubble} fx={mood.fx} /></svg>
    {/if}
  </span>
</span>

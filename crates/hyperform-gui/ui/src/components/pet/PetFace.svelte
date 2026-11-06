<script lang="ts">
  import type { PetFaceParts } from "$lib/domain/pet/emotions";
  import {
    BROWS,
    CHEEKS,
    EYES,
    EYE_GLYPHS,
    EYE_LINES,
    GLINTS,
    MOUTHS,
    SWEAT,
    TEARS,
    isDrawnEye,
    type MouthPaint,
    type Side,
  } from "$lib/domain/pet/faceShapes";

  /**
   * The pet's face in its 32×32 space: cheeks, brows, eyes, mouth, and a
   * sweat drop or tears. Ellipse eyes morph between shapes on a CSS
   * transition; drawn eyes (happy, closed, star, heart, spiral) pop in.
   * The eyes group moves with the gaze (--gx/--gy on the svg).
   */
  type Props = { face: PetFaceParts; blinking?: boolean };
  let { face, blinking = false }: Props = $props();

  const drawn = $derived(isDrawnEye(face.eyes));
  const mouth = $derived(face.mouth === "none" ? [] : MOUTHS[face.mouth]);
  const AT: Record<Side, string> = {
    left: `translate(${EYES.left.cx} ${EYES.left.cy})`,
    right: `translate(${EYES.right.cx} ${EYES.right.cy})`,
  };
  const PAINT: Record<MouthPaint, string> = {
    line: "mouth-line",
    thin: "mouth-line is-thin",
    fill: "mouth-fill",
    tongue: "mouth-tongue",
  };
  const mouthClass = (paint: MouthPaint) =>
    face.mouth === "yawn" || face.mouth === "talk" ? `${PAINT[paint]} mouth-${face.mouth}` : PAINT[paint];
</script>

<g class={`pet-face eyes-${face.eyes} brows-${face.brows} blush-${face.blush}`} class:is-blinking={blinking}>
  <ellipse class="pet-cheek" {...CHEEKS.left} />
  <ellipse class="pet-cheek" {...CHEEKS.right} />
  <path class="pet-brow is-left" d={BROWS.left.d} />
  <path class="pet-brow is-right" d={BROWS.right.d} />

  <g class="pet-eyes">
    {#if !drawn}
      <ellipse class="pet-eye is-left" {...EYES.left} />
      <ellipse class="pet-eye is-right" {...EYES.right} />
      {#if face.eyes === "puppy"}
        <circle class="pet-eye-glint" {...GLINTS.left} />
        <circle class="pet-eye-glint" {...GLINTS.right} />
      {/if}
    {:else if face.eyes === "happy"}
      <path class="pet-eye-line is-drawn" d={EYE_LINES.happy} />
    {:else if face.eyes === "closed"}
      <path class="pet-eye-line is-drawn" d={EYE_LINES.closed} />
    {:else if face.eyes === "star"}
      <g transform={AT.left}><path class="pet-eye-star is-drawn" d={EYE_GLYPHS.star} /></g>
      <g transform={AT.right}><path class="pet-eye-star is-drawn is-late" d={EYE_GLYPHS.star} /></g>
    {:else if face.eyes === "heart"}
      <g transform={AT.left}><path class="pet-eye-heart is-drawn" d={EYE_GLYPHS.heart} /></g>
      <g transform={AT.right}><path class="pet-eye-heart is-drawn is-late" d={EYE_GLYPHS.heart} /></g>
    {:else}
      <g transform={AT.left}><path class="pet-eye-spiral is-drawn" d={EYE_GLYPHS.spiral} /></g>
      <g transform={AT.right}><path class="pet-eye-spiral is-drawn is-late" d={EYE_GLYPHS.spiral} /></g>
    {/if}
  </g>

  {#key face.mouth}
    <g class="pet-mouth">
      {#each mouth as part, i (i)}
        {#if part.kind === "path"}
          <path class={mouthClass(part.paint)} d={part.d} />
        {:else}
          <ellipse class={mouthClass(part.paint)} cx={part.cx} cy={part.cy} rx={part.rx} ry={part.ry} />
        {/if}
      {/each}
    </g>
  {/key}

  {#if face.sweat}
    <path class="pet-sweat" d={SWEAT} />
  {/if}
  {#if face.tears}
    <path class="pet-tear" d={TEARS.left} />
    <path class="pet-tear is-late" d={TEARS.right} />
  {/if}
</g>

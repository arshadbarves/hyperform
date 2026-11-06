<script lang="ts">
  import type { BubbleKind, PetFx } from "$lib/domain/pet/emotions";

  /**
   * What floats above the pet, in its 32×32 space: a little bubble with a
   * glyph (? ! … or an idea), loose zzz, hearts or notes, and bursts of
   * sparkles, confetti or stars circling a dizzy head.
   */
  type Props = { bubble: BubbleKind | null; fx: PetFx };
  let { bubble, fx }: Props = $props();

  const boxed = $derived(bubble === "question" || bubble === "exclaim" || bubble === "dots" || bubble === "idea");
  const STAR = "M0-1.3l.4.9.9.4-.9.4-.4.9-.4-.9-.9-.4.9-.4z";
</script>

{#key bubble}
  {#if boxed}
    <g class={`pet-bubble bubble-${bubble}`}>
      <circle class="bubble-tail" cx="21.9" cy="8.7" r="0.8" />
      <rect class="bubble-body" x="22.6" y="0.6" width="8.8" height="7.4" rx="3.7" />
      {#if bubble === "question"}
        <path class="bubble-glyph" d="M25.8 3.3q0-1.3 1.3-1.3t1.3 1.2q0 .8-.9 1.2-.4.2-.4.8" />
        <circle class="bubble-dot" cx="27" cy="6.3" r="0.45" />
      {:else if bubble === "exclaim"}
        <path class="bubble-glyph" d="M27 1.9v2.9" />
        <circle class="bubble-dot" cx="27" cy="6.3" r="0.5" />
      {:else if bubble === "dots"}
        <circle class="bubble-dot is-typing" cx="25.2" cy="4.3" r="0.6" />
        <circle class="bubble-dot is-typing" cx="27" cy="4.3" r="0.6" />
        <circle class="bubble-dot is-typing" cx="28.8" cy="4.3" r="0.6" />
      {:else}
        <circle class="bubble-bulb" cx="27" cy="3.5" r="1.7" />
        <rect class="bubble-base" x="26.2" y="5.1" width="1.6" height="1.2" rx="0.35" />
      {/if}
    </g>
  {:else if bubble === "zzz"}
    <g class="pet-float pet-zzz">
      <text x="23.4" y="9.4">z</text>
      <text x="26" y="6.2" class="is-late">z</text>
      <text x="28.4" y="3.2" class="is-later">z</text>
    </g>
  {:else if bubble === "hearts"}
    <g class="pet-float pet-hearts">
      <path d="M24.6 9.2c-.9-1.3-2.8-.4-2.2 1.1.4 1 2.2 2.2 2.2 2.2s1.8-1.2 2.2-2.2c.6-1.5-1.3-2.4-2.2-1.1z" />
      <path class="is-late" d="M7.4 7.8c-.7-1-2.2-.3-1.7.9.3.8 1.7 1.7 1.7 1.7s1.4-.9 1.7-1.7c.5-1.2-1-1.9-1.7-.9z" />
      <path class="is-later" d="M27.6 4.4c-.5-.7-1.6-.2-1.2.6.2.6 1.2 1.2 1.2 1.2s1-.6 1.2-1.2c.4-.8-.7-1.3-1.2-.6z" />
    </g>
  {:else if bubble === "notes"}
    <g class="pet-float pet-notes">
      <path d="M24.6 9.6V5.4l2.6-.7v3.6" />
      <circle cx="24" cy="9.7" r="0.8" />
      <circle cx="26.6" cy="8.4" r="0.8" />
      <g class="is-late">
        <path d="M7.4 8.6V5.2" />
        <circle cx="6.8" cy="8.7" r="0.75" />
      </g>
    </g>
  {/if}
{/key}

{#key fx}
  {#if fx === "sparkles"}
    <g class="pet-fx pet-sparkles">
      <path d="M4.6 9l.7 1.6 1.6.7-1.6.7-.7 1.6-.7-1.6-1.6-.7 1.6-.7z" />
      <path d="M26.6 6.4l.6 1.3 1.3.6-1.3.6-.6 1.3-.6-1.3-1.3-.6 1.3-.6z" />
      <path d="M27.4 23.6l.5 1.1 1.1.5-1.1.5-.5 1.1-.5-1.1-1.1-.5 1.1-.5z" />
    </g>
  {:else if fx === "confetti"}
    <g class="pet-fx pet-confetti">
      <rect x="5" y="6" width="1.6" height="1" rx="0.3" />
      <rect x="25" y="5" width="1.6" height="1" rx="0.3" />
      <rect x="9" y="2.5" width="1.2" height="1.2" rx="0.6" />
      <rect x="21.5" y="2" width="1.2" height="1.2" rx="0.6" />
      <rect x="28" y="12" width="1.4" height="0.9" rx="0.3" />
      <rect x="2.6" y="14" width="1.2" height="0.8" rx="0.3" />
    </g>
  {:else if fx === "stars"}
    <g class="pet-fx pet-stars">
      <path d={STAR} transform="translate(22 7)" />
      <path d={STAR} transform="translate(13 12.2)" />
      <path d={STAR} transform="translate(13 1.8)" />
    </g>
  {/if}
{/key}

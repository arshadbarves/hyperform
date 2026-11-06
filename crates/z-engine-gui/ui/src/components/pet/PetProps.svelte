<script lang="ts">
  import type { PetProp } from "$lib/domain/pet/emotions";

  /**
   * What the pet holds for its work, drawn in its own 32×32 space so it
   * moves (and mirrors) with the body: glasses to read, a magnifier to
   * search, a laptop to code, a clipboard to plan, a broom to tidy, a sign
   * to present a plan, and paws clasped to plead.
   */
  type Props = { prop: PetProp | null };
  let { prop }: Props = $props();
</script>

{#key prop}
  {#if prop === "glasses"}
    <g class="pet-prop prop-glasses">
      <circle class="prop-lens" cx="12.4" cy="18.4" r="2.6" />
      <circle class="prop-lens" cx="19.6" cy="18.4" r="2.6" />
      <path class="prop-frame" d="M15 18.1q1-.7 2 0M9.8 17.8l-2.6-1.1M22.2 17.8l2.6-1.1" />
    </g>
  {:else if prop === "magnifier"}
    <g class="pet-prop prop-magnifier">
      <path class="prop-handle" d="M23.4 21.4l3.4 3.8" />
      <circle class="prop-glass" cx="20.8" cy="18.6" r="3.5" />
      <path class="prop-glint" d="M19 16.9q.9-.9 2.1-.8" />
    </g>
  {:else if prop === "laptop"}
    <g class="pet-prop prop-laptop">
      <rect class="prop-lid" x="10.4" y="24.2" width="11.2" height="3.9" rx="0.8" />
      <circle class="prop-logo" cx="16" cy="26.1" r="0.7" />
      <path class="prop-deck" d="M9.2 28.4h13.6" />
    </g>
  {:else if prop === "clipboard"}
    <g class="pet-prop prop-clipboard">
      <rect class="prop-board" x="19.2" y="19.6" width="7.2" height="8.6" rx="0.9" />
      <rect class="prop-paper" x="19.9" y="20.9" width="5.8" height="6.6" rx="0.4" />
      <rect class="prop-clip" x="21.3" y="19" width="3" height="1.5" rx="0.6" />
      <path class="prop-lines" d="M20.8 22.8h3.8M20.8 24.4h4M20.8 26h2.6" />
    </g>
  {:else if prop === "broom"}
    <g class="pet-prop prop-broom">
      <path class="prop-handle" d="M27.2 11.6L21.4 27" />
      <path class="prop-straw" d="M19.2 26.1l4.2 1.5-1.2 3.3q-2.9-.3-4.6-2.2z" />
      <path class="prop-band" d="M19.6 26.3l3.6 1.3" />
    </g>
  {:else if prop === "sign"}
    <g class="pet-prop prop-sign">
      <path class="prop-handle" d="M26.4 29.4V12.6" />
      <rect class="prop-paper prop-board-sign" x="20.6" y="5.2" width="11.2" height="7.8" rx="1.3" />
      <path class="prop-check" d="M22.4 8.1l.9.9 1.5-1.8" />
      <path class="prop-lines" d="M25.8 8.2h4.2M22.6 10.9h7.2" />
    </g>
  {:else if prop === "paws"}
    <g class="pet-prop prop-paws">
      <ellipse class="prop-paw" cx="14.5" cy="25" rx="1.8" ry="1.35" />
      <ellipse class="prop-paw" cx="17.5" cy="25" rx="1.8" ry="1.35" />
    </g>
  {/if}
{/key}

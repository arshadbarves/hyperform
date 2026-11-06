<script lang="ts">
  import type { PetAccessory } from "$lib/domain/pet/looks";

  /** What the pet wears, drawn in the pet's own 32×32 space so it moves with the body. */
  type Props = { kind: PetAccessory };
  let { kind }: Props = $props();
</script>

{#if kind === "sprout"}
  <g class="pet-wear wear-sprout">
    <path class="wear-stem" d="M16 9.2C16 7.8 16.2 6.6 16.9 5.6" />
    <path class="wear-leaf" d="M16.8 5.8C17.6 3.9 19.9 3.3 21.3 4C20.6 5.8 18.6 6.6 16.8 5.8Z" />
    <path class="wear-leaf is-small" d="M16.4 7C15.4 5.6 13.6 5.4 12.6 6.1C13.4 7.4 15 7.8 16.4 7Z" />
  </g>
{:else if kind === "scarf"}
  <g class="pet-wear wear-scarf">
    <path class="wear-band" d="M6.9 22.4C9.4 24.2 12.5 24.9 16 24.9C19.5 24.9 22.6 24.2 25.1 22.4L25.6 24.2C23 26 19.7 26.8 16 26.8C12.3 26.8 9 26 6.4 24.2Z" />
    <path class="wear-tail" d="M21.6 25.2L23.2 29.6L25.4 28.8L24.2 24.6Z" />
  </g>
{:else if kind === "headphones"}
  <g class="pet-wear wear-headphones">
    <path class="wear-arc" d="M7.2 17.4C7.2 11.6 11 8 16 8C21 8 24.8 11.6 24.8 17.4" />
    <rect class="wear-cup" x="4.6" y="15.2" width="3.6" height="5.6" rx="1.8" />
    <rect class="wear-cup" x="23.8" y="15.2" width="3.6" height="5.6" rx="1.8" />
  </g>
{:else}
  <g class="pet-wear wear-star">
    <path
      class="wear-star-shape"
      d="M23.4 3.2l1 2.1 2.3.3-1.7 1.6.4 2.3-2-1.1-2 1.1.4-2.3-1.7-1.6 2.3-.3z"
    />
  </g>
{/if}

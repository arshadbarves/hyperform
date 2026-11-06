<script lang="ts">
  import { petUi } from "$lib/stores/pet.svelte";
  import { Popover } from "$lib/ui";
  import PetCard from "./PetCard.svelte";

  /**
   * The pet card beside the pet: above it while it roams (double-click or
   * right-click it), below the island while it sits in the title bar.
   */
  type Props = { anchor: HTMLElement | null; below: boolean };
  let { anchor, below }: Props = $props();

  const open = $derived(petUi.cardOpen && anchor !== null);
</script>

<Popover.Root
  {open}
  onOpenChange={(next) => {
    if (!next) petUi.cardOpen = false;
  }}
>
  <Popover.Portal>
    <Popover.Content
      class="pet-card glass-thick"
      customAnchor={anchor}
      side={below ? "bottom" : "top"}
      align="center"
      sideOffset={10}
      collisionPadding={12}
    >
      <PetCard onClose={() => (petUi.cardOpen = false)} />
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>

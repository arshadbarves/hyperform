<script lang="ts">
  import type { PetPose } from "$lib/domain/pet/pose";
  import { pet } from "$lib/runtime";
  import { petUi } from "$lib/stores/pet.svelte";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import { perch } from "$lib/ui/perch.svelte";
  import IslandPortrait from "./IslandPortrait.svelte";
  import Pet from "./Pet.svelte";
  import { useLookTarget } from "./petGaze.svelte";

  /**
   * The pet riding in the island: the island's orb slot is its home perch.
   * While it roams elsewhere the slot shows a live portrait of it looking
   * its way, so the island does not change width. While you type it
   * watches the text cursor.
   */
  type Props = { pose: PetPose; progress: number | null; helpers: number };
  let { pose, progress, helpers }: Props = $props();

  const reduced = prefersReducedMotion();
  const listening = $derived(pose.mood === "listening");
  const look = useLookTarget({ pointer: () => false, enabled: () => petUi.docked && listening && !reduced });
</script>

<span class="island-pet" use:perch={{ id: "island", kind: "slot" }}>
  {#if petUi.docked}
    <Pet
      {pose}
      look={petUi.look}
      stage={pet.stage}
      wearing={pet.growth.wearing}
      {progress}
      {helpers}
      size={28}
      lookAt={look.point}
    />
  {:else}
    <IslandPortrait {pose} />
  {/if}
</span>

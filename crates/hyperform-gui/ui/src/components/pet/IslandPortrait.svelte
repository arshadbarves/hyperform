<script lang="ts">
  import { Spring } from "svelte/motion";
  import { portraitLook } from "$lib/domain/pet/portrait";
  import type { PetPose } from "$lib/domain/pet/pose";
  import { pet } from "$lib/runtime";
  import { petUi } from "$lib/stores/pet.svelte";
  import { SPRING, prefersReducedMotion } from "$lib/ui/motion";
  import { perches } from "$lib/ui/perch.svelte";
  import Pet from "./Pet.svelte";

  /**
   * The island's slot while the pet roams: a live portrait of its face with
   * the island's mood on it, turning its head and eyes toward wherever the
   * pet is, in a thin ring of the status tone. Under Reduce Motion it holds
   * still, facing the pet's side. With no WebGL it is the flat pet, cropped.
   */
  type Props = { pose: PetPose };
  let { pose }: Props = $props();

  const SIZE = 22;
  const reduced = prefersReducedMotion();
  const slot = $derived(perches.rects().find((r) => r.id === "island") ?? null);
  const look = $derived(
    slot ? portraitLook({ x: slot.left + slot.width / 2, y: slot.top + slot.height / 2 }, petUi.at) : portraitLook({ x: 0, y: 0 }, null),
  );
  const head = new Spring({ yaw: 0, pitch: 0 }, SPRING.smooth);
  $effect(() => {
    void head.set({ yaw: look.yaw, pitch: look.pitch }, { instant: reduced });
  });
</script>

<span class={`island-portrait tone-${pose.tone}`} aria-hidden="true">
  <Pet
    {pose}
    look={petUi.look}
    stage={pet.stage}
    wearing={pet.growth.wearing}
    size={SIZE}
    framing="portrait"
    head={head.current}
    lookAt={look.lookAt}
  />
</span>

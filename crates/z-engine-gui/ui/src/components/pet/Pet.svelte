<script lang="ts">
  import { expression } from "$lib/domain/pet/emotions";
  import { portraitFor } from "$lib/domain/pet/portrait";
  import PetFlat from "./PetFlat.svelte";
  import { loadPet3D, type Pet3DModule, type PetViewProps } from "./petView";

  /**
   * The pet, in 3D where the webview can draw it (Pet3D) and flat where it
   * cannot (PetFlat): the flat pet also stands in while the 3D one loads
   * and whenever the WebGL context is lost. `framing="portrait"` closes in
   * on its face; the flat pet does that by cropping, so a round window
   * around it shows the same close-up.
   */
  let { turn, lift, bob, framing = "full", head, ...flat }: PetViewProps = $props();

  let gl = $state<Pet3DModule | null>(null);
  $effect(() => {
    let live = true;
    void loadPet3D().then((loaded) => {
      if (live) gl = loaded;
    });
    return () => {
      live = false;
    };
  });

  const size = $derived(flat.size ?? 28);
  const crop = $derived(portraitFor(expression(flat.pose).body, flat.routine ?? null, flat.stage ?? "sprout"));
  const zoom = $derived(32 / crop.span);
</script>

{#if gl && !gl.webgl.lost}
  <gl.View {...flat} {turn} {lift} {bob} {framing} {head} />
{:else if framing === "portrait"}
  <span
    class="pet-crop"
    style:width={`${size}px`}
    style:height={`${size}px`}
    style:--crop-x={`${size / 2 - (size * zoom * crop.cx) / 32}px`}
    style:--crop-y={`${size / 2 - (size * zoom * crop.cy) / 32}px`}
  >
    <PetFlat {...flat} size={size * zoom} />
  </span>
{:else}
  <PetFlat {...flat} />
{/if}

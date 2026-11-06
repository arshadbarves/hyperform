<script lang="ts">
  /** Plan progress as a ring around the pet, 0..1, in its 32-unit space. */
  type Props = { progress: number };
  let { progress }: Props = $props();

  const RING = 2 * Math.PI * 14.6;
</script>

<circle class="pet-ring-track" cx="16" cy="18" r="14.6" />
<circle
  class="pet-ring"
  cx="16"
  cy="18"
  r="14.6"
  stroke-dasharray={RING}
  stroke-dashoffset={RING * (1 - Math.min(1, Math.max(0, progress)))}
/>

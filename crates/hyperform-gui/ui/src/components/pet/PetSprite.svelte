<script lang="ts">
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import type { JobStatus } from "$lib/protocol/JobStatus";

  /**
   * A helper as a tiny sprite of the pet: it bobs while it works, smiles
   * when done, droops when it failed. Decorative; the status word beside it
   * carries the meaning.
   */
  type Props = { status: AgentStatus | JobStatus; size?: number };
  let { status, size = 16 }: Props = $props();

  const happy = $derived(status === "completed");
</script>

<svg class={`pet-sprite-mini status-${status}`} width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
  <circle class="sprite-aura" cx="8" cy="8.6" r="6.6" />
  <circle class="sprite-body" cx="8" cy="8.6" r="5.6" />
  <ellipse class="sprite-shine" cx="6.1" cy="6.2" rx="1.6" ry="0.9" transform="rotate(-28 6.1 6.2)" />
  {#if happy}
    <path class="sprite-eye-happy" d="M5.3 9.2q.9-1.2 1.8 0M8.9 9.2q.9-1.2 1.8 0" />
  {:else}
    <ellipse class="sprite-eye" cx="6.2" cy="8.9" rx="0.7" ry="1.05" />
    <ellipse class="sprite-eye" cx="9.8" cy="8.9" rx="0.7" ry="1.05" />
  {/if}
</svg>

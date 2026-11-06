<script lang="ts">
  import { ui } from "$lib/stores/ui.svelte";
  import Icon, { ListChecks } from "$lib/ui/icons";

  /** A ready plan as one quiet row in the transcript; Review opens it in the Plan tab. */
  type Props = { agentLabel: string | null };
  let { agentLabel }: Props = $props();
</script>

<div class="plan-ready" role="status">
  <Icon icon={ListChecks} size={13} />
  <span class="plan-ready-label">Plan ready</span>
  {#if agentLabel}<span class="plan-ready-agent">{agentLabel}</span>{/if}
  <span class="plan-ready-dot" aria-hidden="true">·</span>
  <button type="button" class="plan-ready-review" data-pending-card onclick={() => ui.openPanel("plan")}>Review</button>
</div>

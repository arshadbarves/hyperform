<script lang="ts">
  import { modeChips } from "$lib/domain/modes";
  import { sessions, setPlanning } from "$lib/runtime";
  import Icon, { ListChecks, X } from "$lib/ui/icons";

  /** While plan mode is on: a chip beside the permission level; × (or Shift+Tab) turns plan mode off. */
  const plan = $derived(modeChips(sessions.active?.planning ?? false, sessions.active?.mode ?? "default").plan);
</script>

{#if plan}
  <span class="composer-plan-chip" title={`${plan.description} (Shift+Tab)`}>
    <Icon icon={ListChecks} size={12} />
    <span>{plan.label}</span>
    <button type="button" class="composer-plan-chip-remove" aria-label={plan.off} title={plan.off} onclick={() => void setPlanning(false)}>
      <Icon icon={X} size={10} strokeWidth={2.2} />
    </button>
  </span>
{/if}

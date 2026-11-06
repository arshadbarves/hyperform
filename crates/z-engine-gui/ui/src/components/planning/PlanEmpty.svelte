<script lang="ts">
  import type { PlanPicker as Picker } from "$lib/domain/work/picker";
  import { setPlanning } from "$lib/runtime";
  import { Button, EmptyState } from "$lib/ui";
  import { ListChecks } from "$lib/ui/icons";
  import PlanPicker from "./PlanPicker.svelte";

  /**
   * The Plan tab with work plans on and no plan attached: start planning,
   * or continue one of the project's plans from other chats.
   */
  type Props = { planning: boolean; picker: Picker };
  let { planning, picker }: Props = $props();

  const any = $derived(picker.open.length + picker.settled.length > 0);
</script>

<div class="plan-view" class:plan-empty={any}>
  <EmptyState
    icon={ListChecks}
    title="No plan yet"
    description={planning
      ? "Plan is on. Say what to build; the agent researches and proposes tasks here, and nothing changes until you approve."
      : "Plan first and the agent researches, then proposes tasks here. Nothing changes until you approve."}
  >
    {#if !planning}<Button size="s" onclick={() => void setPlanning(true)}>Start planning</Button>{/if}
  </EmptyState>
  {#if any}<PlanPicker {picker} />{/if}
</div>

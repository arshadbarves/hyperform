<script lang="ts">
  import type { PlanTrace, TraceKind } from "$lib/domain/work/trace";
  import { ui } from "$lib/stores/ui.svelte";
  import Icon, { AlertOctagon, AlertTriangle, ArrowUpRight, CheckCircle2, GitMerge, Play, type IconSvgElement } from "$lib/ui/icons";

  /**
   * One quiet line in the transcript for something the plan did: approved,
   * revised, resumed after a restart or an attach, a task failed or
   * blocked, or moved to another chat. Why opens the change in the Plan
   * tab's notebook; Open opens the task.
   */
  let { trace }: { trace: PlanTrace } = $props();

  const ICON: Record<TraceKind, IconSvgElement> = {
    approved: CheckCircle2,
    revised: GitMerge,
    resumed: Play,
    failed: AlertOctagon,
    blocked: AlertTriangle,
    moved: ArrowUpRight,
  };

  function why(planId: string) {
    ui.openPlan(planId, {
      view: "tasks",
      selected: null,
      notebookOpen: true,
      notebookTab: "changes",
      focusEntry: trace.entryId,
    });
  }

  function open(planId: string) {
    ui.openPlan(planId, { view: "tasks", selected: trace.taskId });
  }
</script>

<div class={`plan-trace tone-${trace.tone}`}>
  <Icon icon={ICON[trace.kind]} size={11} class="plan-trace-icon" />
  <span class="plan-trace-text">{trace.text}</span>
  {#if trace.detail}<span class="plan-trace-detail">· {trace.detail}</span>{/if}
  {#if trace.planId}
    {@const planId = trace.planId}
    {#if trace.entryId}<button type="button" class="plan-trace-link" onclick={() => why(planId)}>Why</button>{/if}
    {#if trace.taskId}<button type="button" class="plan-trace-link" onclick={() => open(planId)}>Open</button>{/if}
  {/if}
</div>

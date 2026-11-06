<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import { plural } from "$lib/domain/format";
  import { needLine, needsStrip, needsYouItems, type NeedItem } from "$lib/domain/work/needsYou";
  import { WEB_HINT } from "$lib/domain/work/status";
  import type { WorkEdit } from "$lib/protocol/WorkEdit";
  import { editWork } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { revealWorktree } from "$lib/stores/plan-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { revealLabel } from "$lib/platform";

  /**
   * What the plan waits on you for, most urgent first: merge conflicts,
   * changes ready to apply, the tasks a plan change holds (accepting it is
   * in the change set below), tasks that stopped, verifying tasks whose
   * checks did not run or are stale, proposed requirements and tasks, open
   * high-severity audit findings (medium and low ones stay in the findings
   * list). Nothing shows while nothing waits, and nothing the plan
   * waits on folds; findings past the first two are a count that opens the
   * findings list. Approvals and questions stay in the composer.
   */
  type Props = { work: WorkView; onSelect: (taskId: string) => void; onFindings: () => void };
  let { work, onSelect, onFindings }: Props = $props();

  const strip = $derived(needsStrip(needsYouItems(work)));
  const planId = $derived(work.plan.planId);

  function edit(change: WorkEdit) {
    void editWork(planId, change);
  }

  async function notNeeded(taskId: string, title: string) {
    const ok = await confirmStore.ask({
      title: "Mark this task not needed?",
      description: `“${title}” is cancelled; tasks that wait on it can go ahead.`,
      confirmLabel: "Not needed",
      tone: "danger",
    });
    if (ok) edit({ type: "cancelTask", taskId });
  }

  function open(need: NeedItem) {
    if (need.kind === "finding") onFindings();
    else if (need.kind === "replan") onSelect(need.tasks[0].id);
    else if (need.kind !== "item") onSelect(need.task.id);
  }
</script>

{#if strip.shown.length}
  <section class="plan-needs" aria-label="Needs you">
    <ul class="plan-needs-list">
      {#each strip.shown as need (need.key)}
        <li class="plan-needs-item">
          {#if need.kind === "item"}
            <span class="plan-needs-text">{needLine(need)}</span>
          {:else}
            <button type="button" class="plan-needs-text" onclick={() => open(need)}>{needLine(need)}</button>
          {/if}
          {#if need.kind === "conflict" && need.paths.length}
            <span class="plan-needs-detail">{need.paths.join(", ")}</span>
          {/if}
          {#if need.kind === "replan"}
            <span class="plan-needs-detail is-tasks">
              {#each need.tasks as held, i (held.id)}
                <button type="button" class="task-link" onclick={() => onSelect(held.id)}>{held.title}</button>{i < need.tasks.length - 1 ? ", " : ""}
              {/each}
            </span>
          {/if}
          {#if (need.kind === "item" && need.item.provenance === "research") || (need.kind === "task" && need.task.provenance === "research")}
            <span class="plan-needs-detail" title={WEB_HINT}>from web</span>
          {/if}
          <span class="plan-needs-actions">
            {#if need.kind === "conflict"}
              <button type="button" class="task-link" onclick={() => edit({ type: "retrySerially", taskId: need.task.id })}>
                Retry serially
              </button>
              {#if need.worktree}
                {@const path = need.worktree}
                <button type="button" class="task-link" title={revealLabel()} onclick={() => void revealWorktree(path)}>Show worktree</button>
              {/if}
            {:else if need.kind === "apply"}
              <button type="button" class="task-link" onclick={() => ui.openPanel("agents")}>Review</button>
            {:else if need.kind === "retry"}
              <button type="button" class="task-link" onclick={() => edit({ type: "retryTask", taskId: need.task.id })}>Retry</button>
              <button type="button" class="task-link" onclick={() => void notNeeded(need.task.id, need.task.title)}>Not needed</button>
            {:else if need.kind === "recheck"}
              <button type="button" class="task-link" onclick={() => edit({ type: "recheckTask", taskId: need.task.id })}>
                Run checks again
              </button>
            {:else if need.kind === "item"}
              <button type="button" class="task-link" onclick={() => edit({ type: "acceptItem", itemId: need.item.id })}>Accept</button>
              <button type="button" class="task-link" onclick={() => edit({ type: "retireItem", itemId: need.item.id })}>Dismiss</button>
            {:else if need.kind === "task"}
              <button type="button" class="task-link" onclick={() => edit({ type: "acceptTask", taskId: need.task.id })}>Accept</button>
              <button type="button" class="task-link" onclick={() => edit({ type: "cancelTask", taskId: need.task.id })}>Dismiss</button>
            {:else if need.kind === "finding"}
              <button type="button" class="task-link" onclick={() => edit({ type: "addFinding", findingId: need.finding.id })}>
                Add to plan
              </button>
              <button type="button" class="task-link" onclick={() => edit({ type: "dismissFinding", findingId: need.finding.id })}>
                Dismiss
              </button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
    {#if strip.moreFindings}
      <button type="button" class="task-link plan-needs-more" onclick={onFindings}>
        {plural(strip.moreFindings, "more finding")}
      </button>
    {/if}
  </section>
{/if}

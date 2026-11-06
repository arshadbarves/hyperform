<script lang="ts">
  import { criterionLines, type CriterionLine } from "$lib/domain/work/criteria";
  import type { ReconcileFinding } from "$lib/protocol/ReconcileFinding";
  import type { Task } from "$lib/protocol/Task";
  import { editWork } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Pill } from "$lib/ui";
  import { relTime } from "$lib/util";
  import { PILL_TONE } from "./taskGlyph";

  /**
   * A task's acceptance criteria: each with how it is shown to hold, its
   * status and its evidence (a check's result and when, stale once the
   * tree changed since). You confirm your own checks and waive what you
   * decide not to need.
   */
  type Props = { planId: string; task: Task; findings: readonly ReconcileFinding[]; readOnly: boolean };
  let { planId, task, findings, readOnly }: Props = $props();

  const lines = $derived(criterionLines(task, findings));

  async function waive(line: CriterionLine) {
    const ok = await confirmStore.ask({
      title: "Waive this criterion?",
      description: `“${line.text}” will no longer be needed for “${task.title}” to be done.`,
      confirmLabel: "Waive",
    });
    if (ok) void editWork(planId, { type: "waiveCriterion", taskId: task.id, criterionId: line.id });
  }

  function confirm(line: CriterionLine) {
    void editWork(planId, { type: "confirmCriterion", taskId: task.id, criterionId: line.id });
  }
</script>

{#if lines.length}
  <section class="task-section" aria-label="Acceptance criteria">
    <h3 class="plan-view-heading">Criteria</h3>
    <ul class="task-criteria">
      {#each lines as line (line.id)}
        <li class={`task-criterion tone-${line.tone}`}>
          <div class="task-criterion-head">
            <span class="task-criterion-text">{line.text}</span>
            <Pill plain dot tone={PILL_TONE[line.tone]}>{line.label}</Pill>
          </div>
          <div class="task-criterion-meta">
            <span>{line.method}</span>
            {#if !readOnly && line.canConfirm}
              <button type="button" class="task-link" onclick={() => confirm(line)}>Confirm</button>
            {/if}
            {#if !readOnly && line.canWaive}
              <button type="button" class="task-link" onclick={() => void waive(line)}>Waive</button>
            {/if}
          </div>
          {#if line.evidence.length}
            <ul class="task-evidence">
              {#each line.evidence as ev (ev.key)}
                <li class:is-failed={ev.passed === false} class:is-passed={ev.passed === true}>
                  <span class="task-evidence-text">{ev.text}</span>
                  {#if ev.at}<time class="task-evidence-time">{relTime(ev.at)}</time>{/if}
                  {#if ev.stale}<span class="task-evidence-stale" title="Files changed since this ran">stale</span>{/if}
                  {#if ev.files.length}
                    <button type="button" class="task-link" onclick={() => ui.openPanel("changes", ev.files[0], "session")}>
                      Changes
                    </button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

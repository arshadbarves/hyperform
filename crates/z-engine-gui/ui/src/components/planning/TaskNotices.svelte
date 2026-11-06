<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import { needsRecheck } from "$lib/domain/work/criteria";
  import { holdEdit, replanHeldTasks } from "$lib/domain/work/held";
  import { resumeNote } from "$lib/domain/work/resume";
  import { fromWeb, type TaskActions, type TaskLook } from "$lib/domain/work/status";
  import { revealLabel } from "$lib/platform";
  import type { Task } from "$lib/protocol/Task";
  import type { WorkEdit } from "$lib/protocol/WorkEdit";
  import { sessions } from "$lib/runtime";
  import { revealWorktree } from "$lib/stores/plan-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button } from "$lib/ui";

  /**
   * What a task's detail says under its status: why it is in that state;
   * Accept and Dismiss while proposed; Run checks again while verifying on
   * checks that did not run or are stale; your hold, with Release; that it
   * waits for the plan change; and, when it was interrupted, that its
   * helper's work is kept, with Show worktree.
   */
  type Props = {
    work: WorkView;
    task: Task;
    look: TaskLook;
    can: TaskActions;
    readOnly: boolean;
    onEdit: (change: WorkEdit) => void;
  };
  let { work, task, look, can, readOnly, onEdit }: Props = $props();

  const recheck = $derived(!readOnly && needsRecheck(task, work.findings));
  const forChange = $derived(!task.held && replanHeldTasks(work).some((t) => t.id === task.id));
  const resume = $derived(resumeNote(task));
  const transcript = $derived(resume?.agentId && resume.sessionId === sessions.activeId ? resume.agentId : null);
</script>

{#if look.detail}<p class={`task-detail-why tone-${look.tone}`}>{look.detail}</p>{/if}
{#if can.accept}
  <div class="task-detail-proposal">
    <p class="task-detail-why tone-attention">
      {fromWeb(task) ? "Proposed by a turn that read web content; it waits for you." : "Proposed; it waits for you."}
    </p>
    <Button size="s" variant="secondary" onclick={() => onEdit({ type: "acceptTask", taskId: task.id })}>Accept</Button>
    <Button size="s" onclick={() => onEdit({ type: "cancelTask", taskId: task.id })}>Dismiss</Button>
  </div>
{/if}
{#if recheck}
  <div class="task-detail-proposal">
    <p class="task-detail-why tone-attention">A check did not run or its result is stale, so the task cannot finish verifying.</p>
    <Button size="s" variant="secondary" onclick={() => onEdit({ type: "recheckTask", taskId: task.id })}>Run checks again</Button>
  </div>
{/if}
{#if task.held}
  <div class="task-detail-proposal">
    <p class="task-detail-why tone-quiet">
      {task.status.type === "active"
        ? "On hold: its helper stops at its next step, and it won't start again until you release it."
        : "On hold: it won't start until you release it."}
    </p>
    {#if can.release}<Button size="s" onclick={() => onEdit(holdEdit(task.id, false))}>Release</Button>{/if}
  </div>
{:else if forChange}
  <p class="task-detail-why tone-quiet">
    Waits for the plan change: it starts once you accept the plan's changes. The other ready tasks keep running.
  </p>
{/if}
{#if resume}
  <div class="task-detail-proposal">
    <p class="task-detail-why tone-quiet">
      {resume.text}{resume.note ? ` · ${resume.note}` : ""}
    </p>
    {#if resume.worktree}
      {@const path = resume.worktree}
      <Button size="s" title={revealLabel()} onclick={() => void revealWorktree(path)}>Show worktree</Button>
    {/if}
    {#if transcript}
      {@const agentId = transcript}
      <button type="button" class="task-link" onclick={() => ui.openPanel("agents", agentId)}>Its helper's transcript</button>
    {/if}
  </div>
{/if}

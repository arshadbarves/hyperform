<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import { changeSet, type ChangeKind } from "$lib/domain/work/changes";
  import { replanHeldTasks, replanWaitLine } from "$lib/domain/work/held";
  import { keepPlanning } from "$lib/runtime";
  import { startBuild } from "$lib/stores/app-actions";
  import { Button } from "$lib/ui";
  import PlanFeedback from "./PlanFeedback.svelte";

  /**
   * Replanning: what changed since you approved, the tasks it touched (each
   * opens), the reasons recorded for it, and the tasks it holds until you
   * accept (after a failure or an overlap; the other ready tasks keep
   * running). Accept changes approves the new revision and building goes
   * on; Keep planning sends feedback.
   */
  type Props = { work: WorkView; requestId: string | null; onSelect: (taskId: string) => void };
  let { work, requestId, onSelect }: Props = $props();

  const SHOWN = 6;
  const KIND: Record<ChangeKind, string> = {
    added: "Added",
    reopened: "Reopened",
    replaced: "Replaced",
    removed: "Removed",
    changed: "Changed",
  };

  const set = $derived(changeSet(work));
  const held = $derived(replanHeldTasks(work));
  const reasons = $derived(
    set.changes.flatMap((e) => (e.body.type === "planChange" && e.body.reason ? [e.body.reason] : [])).slice(0, 2),
  );
  let busy = $state(false);
  let revising = $state(false);

  async function run(job: () => Promise<boolean>) {
    if (busy) return;
    busy = true;
    try {
      if (await job()) revising = false;
    } finally {
      busy = false;
    }
  }

  const accept = () =>
    run(() => startBuild({ planId: work.plan.planId, projectRoot: work.plan.projectRoot, target: "here", requestId }));
</script>

<footer class="plan-view-actions plan-changeset" aria-label="Changes to approve">
  <p class="plan-changeset-line">
    <strong>Since you approved</strong> (rev {set.since}): {set.line || "no task changed"}
  </p>
  {#if set.tasks.length}
    <ul class="plan-changeset-tasks">
      {#each set.tasks.slice(0, SHOWN) as change (change.task.id)}
        <li>
          <button type="button" class="plan-changeset-task" onclick={() => onSelect(change.task.id)}>
            <span class={`plan-changeset-kind is-${change.kind}`}>{KIND[change.kind]}</span>
            <span class="plan-changeset-title">{change.task.title}</span>
          </button>
        </li>
      {/each}
      {#if set.tasks.length > SHOWN}<li class="plan-changeset-more">and {set.tasks.length - SHOWN} more</li>{/if}
    </ul>
  {/if}
  {#each reasons as reason, i (i)}<p class="plan-changeset-reason">{reason}</p>{/each}
  {#if held.length}
    <p class="plan-changeset-held">
      {replanWaitLine(held.length)}:
      {#each held as task, i (task.id)}
        <button type="button" class="task-link" onclick={() => onSelect(task.id)}>{task.title}</button>{i < held.length - 1 ? ", " : ""}
      {/each}. The other ready tasks keep running.
    </p>
  {/if}
  {#if revising}
    <PlanFeedback {busy} onSend={(text) => void run(() => keepPlanning(requestId, text))} onCancel={() => (revising = false)} />
  {:else}
    <div class="plan-view-buttons">
      <Button variant="accent" disabled={busy} onclick={() => void accept()}>Accept changes</Button>
      <Button disabled={busy} onclick={() => (revising = true)}>Keep planning</Button>
    </div>
  {/if}
</footer>

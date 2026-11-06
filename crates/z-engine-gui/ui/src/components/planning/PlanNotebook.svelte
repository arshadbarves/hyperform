<script lang="ts">
  import { tick } from "svelte";
  import type { WorkView } from "$lib/domain/sessionView";
  import type { PlanViewState } from "$lib/domain/work/panel";
  import { NOTEBOOK_TABS, notebook, notebookSummary } from "$lib/domain/work/notebook";
  import { Disclosure, SegmentedChoice } from "$lib/ui";
  import NotebookEntry from "./NotebookEntry.svelte";

  /**
   * What the plan learned on the way, folded by default with its counts:
   * design decisions, discoveries, experiments and changes of plan, one
   * tab each.
   */
  type Props = {
    work: WorkView;
    place: PlanViewState;
    readOnly: boolean;
    onState: (patch: Partial<PlanViewState>) => void;
  };
  let { work, place, readOnly, onState }: Props = $props();

  const groups = $derived(notebook(work.entries));
  const entries = $derived(groups[place.notebookTab]);
  const counts = $derived(notebookSummary(groups));
  const unconfirmed = $derived(
    Object.values(groups)
      .flat()
      .filter((e) => e.status.type === "proposed").length,
  );

  // A focused entry (a trace's Why) opens once; then it is an ordinary line again.
  $effect(() => {
    if (place.focusEntry) void tick().then(() => onState({ focusEntry: null }));
  });
</script>

<Disclosure
  class="plan-notebook"
  summaryClass="plan-notebook-summary"
  open={place.notebookOpen}
  onOpenChange={(open) => onState({ notebookOpen: open })}
>
  {#snippet summary()}
    <span class="plan-view-heading">Notebook</span>
    <span class="plan-notebook-counts">{counts}</span>
    {#if unconfirmed}<span class="plan-notebook-unconfirmed">{unconfirmed} unconfirmed</span>{/if}
  {/snippet}
  <SegmentedChoice
    compact
    label="Notebook section"
    options={NOTEBOOK_TABS}
    value={place.notebookTab}
    onSelect={(tab) => onState({ notebookTab: tab })}
  />
  {#if entries.length === 0}
    <p class="task-note">Nothing here yet.</p>
  {:else}
    <div class="plan-notebook-list">
      {#each entries as entry (entry.id)}
        <NotebookEntry planId={work.plan.planId} {entry} tasks={work.tasks} {readOnly} focused={entry.id === place.focusEntry} />
      {/each}
    </div>
  {/if}
</Disclosure>

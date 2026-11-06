<script lang="ts">
  import { untrack } from "svelte";
  import { entryLine } from "$lib/domain/work/notebook";
  import { WEB_HINT, type TaskMap } from "$lib/domain/work/status";
  import type { Entry } from "$lib/protocol/Entry";
  import { editWork } from "$lib/runtime";
  import { Disclosure, Pill } from "$lib/ui";

  /**
   * One notebook entry as a line that unfolds: a decision to its choice,
   * alternatives and reason; an experiment to its method, result and
   * conclusion; a change to its from and to, evidence and tasks. Entries
   * nobody confirmed yet (extracted while compacting, or recorded by a turn
   * that read web content) say so, with Confirm and Dismiss; web entries
   * keep a quiet "from web" mark.
   */
  type Props = { planId: string; entry: Entry; tasks: TaskMap; readOnly: boolean; focused?: boolean };
  let { planId, entry, tasks, readOnly, focused = false }: Props = $props();

  const line = $derived(entryLine(entry, tasks));
  let open = $state(untrack(() => focused));
  let root: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!focused || !root) return;
    open = true;
    root.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="notebook-entry" bind:this={root}>
  <Disclosure summaryClass="notebook-entry-summary" bind:open>
    {#snippet summary()}
      <span class="notebook-entry-title">{line.title}</span>
      {#if line.tag}<span class="notebook-entry-tag">{line.tag}</span>{/if}
      {#if line.fromWeb}<span class="notebook-entry-tag" title={WEB_HINT}>from web</span>{/if}
      {#if line.unconfirmed}<Pill plain dot tone="attention">Unconfirmed</Pill>{/if}
    {/snippet}
    {#snippet actions()}
      {#if line.unconfirmed && !readOnly}
        <button type="button" class="task-link" onclick={() => void editWork(planId, { type: "confirmEntry", entryId: entry.id })}>
          Confirm
        </button>
        <button type="button" class="task-link" onclick={() => void editWork(planId, { type: "dismissEntry", entryId: entry.id })}>
          Dismiss
        </button>
      {/if}
    {/snippet}
    {#if line.facts.length}
      <dl class="notebook-entry-facts">
        {#each line.facts as fact (fact.label)}
          <dt>{fact.label}</dt>
          <dd>{fact.value}</dd>
        {/each}
      </dl>
    {/if}
    {#if line.source}<p class="task-note">{line.source}</p>{/if}
  </Disclosure>
</div>

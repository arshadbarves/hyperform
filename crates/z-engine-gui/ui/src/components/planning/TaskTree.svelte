<script lang="ts">
  import { tick } from "svelte";
  import type { HoldBadge } from "$lib/domain/work/held";
  import type { ParallelBadge } from "$lib/domain/work/parallel";
  import { treeKeyAction, type TreeRow } from "$lib/domain/work/tree";
  import TaskRow from "./TaskRow.svelte";

  /**
   * The plan's tasks as a tree (WAI-ARIA tree pattern, flat with levels):
   * arrows move and fold, Home and End jump, Enter opens a task's detail.
   * One row holds the tab stop.
   */
  type Props = {
    rows: TreeRow[];
    replaced: number;
    showReplaced: boolean;
    selected: string | null;
    onSelect: (taskId: string) => void;
    onToggle: (taskId: string, open: boolean) => void;
    onShowReplaced: (show: boolean) => void;
    /** Accepts a proposed task; absent while the plan is read-only. */
    onAccept?: (taskId: string) => void;
    /** Whether each task can run in a parallel helper, while `work_parallel` is effective. */
    badges?: Readonly<Record<string, ParallelBadge>>;
    /** Tasks held by you or for the plan change. */
    holds?: Readonly<Record<string, HoldBadge>>;
  };
  let {
    rows,
    replaced,
    showReplaced,
    selected,
    onSelect,
    onToggle,
    onShowReplaced,
    onAccept,
    badges = {},
    holds = {},
  }: Props = $props();

  let tree: HTMLDivElement | undefined = $state();
  let focus = $state(0);
  const focusIndex = $derived(Math.min(focus, Math.max(0, rows.length - 1)));

  async function focusRow(index: number) {
    focus = index;
    await tick();
    tree?.querySelectorAll<HTMLElement>('[role="treeitem"]')[index]?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.target !== tree && !(e.target as HTMLElement).matches('[role="treeitem"]')) return;
    const action = treeKeyAction(rows, focusIndex, e.key);
    if (!action) return;
    e.preventDefault();
    if (action.type === "focus") void focusRow(action.index);
    else if (action.type === "expand") onToggle(action.id, action.expanded);
    else onSelect(action.id);
  }
</script>

{#if rows.length === 0}
  <p class="task-tree-empty">No tasks yet. The agent adds them as it plans.</p>
{:else}
  <div class="task-tree" role="tree" aria-label="Tasks" tabindex="-1" bind:this={tree} {onkeydown}>
    {#each rows as row, i (row.task.id)}
      <TaskRow
        {row}
        selected={row.task.id === selected}
        focused={i === focusIndex}
        onSelect={() => {
          focus = i;
          onSelect(row.task.id);
        }}
        onToggle={() => onToggle(row.task.id, !row.expanded)}
        onFocus={() => (focus = i)}
        onAccept={onAccept && (() => onAccept(row.task.id))}
        parallel={badges[row.task.id] ?? null}
        hold={holds[row.task.id] ?? null}
      />
    {/each}
  </div>
{/if}
{#if replaced > 0}
  <button type="button" class="task-tree-replaced" onclick={() => onShowReplaced(!showReplaced)}>
    {showReplaced ? "Hide replaced tasks" : `Show ${replaced} replaced`}
  </button>
{/if}

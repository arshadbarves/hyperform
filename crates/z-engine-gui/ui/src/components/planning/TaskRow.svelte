<script lang="ts">
  import type { HoldBadge } from "$lib/domain/work/held";
  import type { ParallelBadge } from "$lib/domain/work/parallel";
  import { WEB_HINT, fromWeb } from "$lib/domain/work/status";
  import type { TreeRow } from "$lib/domain/work/tree";
  import Icon, { ChevronRight } from "$lib/ui/icons";
  import { TASK_GLYPH } from "./taskGlyph";

  /**
   * One task in the tree: a fold chevron for groups, the status glyph, the
   * title, a quiet "on hold" (or "waits for the plan change") badge, a
   * quiet "from web" mark, whether it can run beside others, then
   * its dependency badge and either its children's progress or why it is
   * in its state. A proposed task
   * offers Accept by pointer; the detail has it for the keyboard. The tree
   * owns focus and keys.
   */
  type Props = {
    row: TreeRow;
    selected: boolean;
    focused: boolean;
    onSelect: () => void;
    onToggle: () => void;
    onFocus: () => void;
    /** Absent while the plan is read-only. */
    onAccept?: () => void;
    /** Whether it can run beside others, or why it runs alone; replaces the plain "runs alone". */
    parallel?: ParallelBadge | null;
    /** Held by you, or until you accept the plan change. */
    hold?: HoldBadge | null;
  };
  let { row, selected, focused, onSelect, onToggle, onFocus, onAccept, parallel = null, hold = null }: Props = $props();

  const look = $derived(row.look);
  const aside = $derived(
    row.progress && !row.expanded
      ? `${row.progress.done} of ${row.progress.total}`
      : look.state === "failed" || look.state === "blocked"
        ? [look.label.toLowerCase(), look.detail].filter(Boolean).join(" · ")
        : null,
  );
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class={`task-row tone-${look.tone}`}
  class:is-selected={selected}
  class:is-done={look.state === "done" || look.state === "cancelled" || look.state === "superseded"}
  role="treeitem"
  aria-level={row.depth + 1}
  aria-setsize={row.size}
  aria-posinset={row.pos}
  aria-expanded={row.hasChildren ? row.expanded : undefined}
  aria-selected={selected}
  tabindex={focused ? 0 : -1}
  data-task={row.task.id}
  style:--depth={row.depth}
  onclick={onSelect}
  onfocus={onFocus}
>
  {#if row.hasChildren}
    <span
      class="task-row-fold"
      class:is-open={row.expanded}
      class:is-locked={!row.canCollapse}
      aria-hidden="true"
      onclick={(e) => {
        e.stopPropagation();
        if (row.canCollapse || !row.expanded) onToggle();
      }}
    >
      <Icon icon={ChevronRight} size={11} strokeWidth={2} />
    </span>
  {:else}
    <span class="task-row-fold" aria-hidden="true"></span>
  {/if}
  <span class="task-row-glyph" class:is-live={look.state === "active"} title={look.label}>
    <Icon icon={TASK_GLYPH[look.state]} size={13} />
  </span>
  <span class="task-row-title">{row.task.title}</span>
  {#if hold}<span class="task-row-badge is-held" title={hold.title}>{hold.text}</span>{/if}
  {#if fromWeb(row.task)}<span class="task-row-badge" title={WEB_HINT}>from web</span>{/if}
  {#if parallel}
    <span class="task-row-badge" class:is-together={parallel.together} title={parallel.title}>{parallel.text}</span>
  {:else if row.task.runAlone}
    <span class="task-row-badge">runs alone</span>
  {/if}
  {#if row.deps}<span class="task-row-badge">{row.deps}</span>{/if}
  {#if aside}<span class="task-row-aside">{aside}</span>{/if}
  {#if onAccept && look.state === "proposed"}
    <button
      type="button"
      class="task-link task-row-accept"
      tabindex="-1"
      title="Accept this proposed task"
      onclick={(e) => {
        e.stopPropagation();
        onAccept();
      }}>Accept</button
    >
  {/if}
  <span class="sr-only">{look.label}{look.detail ? `, ${look.detail}` : ""}</span>
</div>

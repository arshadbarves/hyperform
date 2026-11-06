<script lang="ts">
  import { findingKind, findingStatus, type FindingRef } from "$lib/domain/work/health";
  import type { AuditFinding } from "$lib/protocol/AuditFinding";

  /**
   * One audit finding: what kind of gap, the finding and its suggested
   * fix, what it is about (tasks open their detail), and while it is open
   * and the plan editable, Add to plan and Dismiss.
   */
  type Props = {
    finding: AuditFinding;
    refs: FindingRef[];
    onSelect: (taskId: string) => void;
    /** Absent while the plan is read-only. */
    onAdd?: () => void;
    onDismiss?: () => void;
  };
  let { finding, refs, onSelect, onAdd, onDismiss }: Props = $props();

  const settled = $derived(findingStatus(finding.status));
</script>

<li class={`finding-row severity-${finding.severity}`}>
  <p class="finding-row-head">
    <span>{findingKind(finding.kind)}</span>
    {#if finding.source === "auditAgent"}<span>audit agent</span>{/if}
    {#if settled}<span>{settled}</span>{/if}
  </p>
  <p class="finding-row-text">{finding.text}</p>
  {#if finding.suggestion}<p class="finding-row-suggestion">Suggested: {finding.suggestion}</p>{/if}
  {#if refs.length}
    <p class="finding-row-refs">
      {#each refs as ref (ref.key)}
        {#if ref.taskId}
          {@const taskId = ref.taskId}
          <button type="button" class="task-link" onclick={() => onSelect(taskId)}>{ref.label}</button>
        {:else}
          <span class="finding-row-ref">{ref.label}</span>
        {/if}
      {/each}
    </p>
  {/if}
  {#if finding.status === "open" && onAdd && onDismiss}
    <p class="finding-row-actions">
      <button type="button" class="task-link" onclick={onAdd}>Add to plan</button>
      <button type="button" class="task-link" onclick={onDismiss}>Dismiss</button>
    </p>
  {/if}
</li>

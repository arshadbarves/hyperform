<script lang="ts">
  import type { WorkView } from "$lib/domain/sessionView";
  import { coverageLine, findingGroups, findingRefs } from "$lib/domain/work/health";
  import type { AuditFinding } from "$lib/protocol/AuditFinding";
  import { editWork } from "$lib/runtime";
  import { Button, Disclosure } from "$lib/ui";
  import Icon, { ChevronLeft } from "$lib/ui/icons";
  import FindingRow from "./FindingRow.svelte";

  /**
   * The audit's findings, opened from the header's coverage line: open
   * findings by severity with Add to plan and Dismiss, then what was added
   * to the plan; dismissed and resolved ones fold away. Run audit asks the
   * engine for a fresh audit. Esc or Back returns to the tasks.
   */
  type Props = { work: WorkView; readOnly: boolean; onBack: () => void; onSelect: (taskId: string) => void };
  let { work, readOnly, onBack, onSelect }: Props = $props();

  const groups = $derived(findingGroups(work.audit));
  const summary = $derived(work.health ? coverageLine(work.health, work.audit).text : null);
  const planId = $derived(work.plan.planId);
  let settledOpen = $state(false);
  let asking = $state(false);

  async function runAudit() {
    asking = true;
    await editWork(planId, { type: "requestAudit" });
    asking = false;
  }

  const refs = (finding: AuditFinding) => findingRefs(finding, work.tasks, work.plan);
  const add = (finding: AuditFinding) => () => void editWork(planId, { type: "addFinding", findingId: finding.id });
  const dismiss = (finding: AuditFinding) => () =>
    void editWork(planId, { type: "dismissFinding", findingId: finding.id });
</script>

<article class="plan-findings" aria-label="Findings">
  <header class="task-detail-head">
    <Button variant="icon" aria-label="Back to tasks" title="Back to tasks · Esc" onclick={onBack}>
      <Icon icon={ChevronLeft} size={15} />
    </Button>
    <h2 class="task-detail-title">Findings</h2>
    <span class="plan-view-space"></span>
    {#if !readOnly}
      <Button size="s" disabled={asking} onclick={() => void runAudit()}>Run audit</Button>
    {/if}
  </header>
  {#if summary}<p class="task-detail-meta">{summary}</p>{/if}

  {#if groups.open.length === 0}
    <p class="task-note">No open findings. The audit checks that every requirement has a task with criteria.</p>
  {/if}
  {#each groups.open as group (group.severity)}
    <section class="task-section" aria-label={`${group.label} severity`}>
      <h3 class="plan-view-heading">{group.label}</h3>
      <ul class="finding-list">
        {#each group.findings as finding (finding.id)}
          <FindingRow
            {finding}
            refs={refs(finding)}
            {onSelect}
            onAdd={readOnly ? undefined : add(finding)}
            onDismiss={readOnly ? undefined : dismiss(finding)}
          />
        {/each}
      </ul>
    </section>
  {/each}

  {#if groups.added.length}
    <section class="task-section" aria-label="Added to the plan">
      <h3 class="plan-view-heading">Added to the plan</h3>
      <ul class="finding-list">
        {#each groups.added as finding (finding.id)}<FindingRow {finding} refs={refs(finding)} {onSelect} />{/each}
      </ul>
    </section>
  {/if}

  {#if groups.settled.length}
    <Disclosure bind:open={settledOpen} class="task-notes" summaryClass="task-notes-summary">
      {#snippet summary()}<span class="plan-view-heading">Dismissed and resolved · {groups.settled.length}</span>{/snippet}
      <ul class="finding-list">
        {#each groups.settled as finding (finding.id)}<FindingRow {finding} refs={refs(finding)} {onSelect} />{/each}
      </ul>
    </Disclosure>
  {/if}
</article>

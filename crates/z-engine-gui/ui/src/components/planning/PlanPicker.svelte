<script lang="ts">
  import type { PickerRow, PlanPicker } from "$lib/domain/work/picker";
  import { chatTitle } from "$lib/runtime";
  import { pickPlan } from "$lib/stores/plan-actions";
  import { Disclosure } from "$lib/ui";
  import Icon, { ChevronLeft } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import { relTime } from "$lib/util";

  /**
   * "Continue a plan": the project's other plans, newest first, with their
   * status, progress, what needs you and when they last changed. Done and
   * abandoned plans wait behind a disclosure. Choosing one attaches it to
   * this chat; one another chat has moves here once you confirm.
   */
  type Props = {
    picker: PlanPicker;
    /** With a plan attached: back to it. */
    onBack?: () => void;
  };
  let { picker, onBack }: Props = $props();

  const clock = ticker(() => true, 30_000);
  let busy = $state(false);
  let settledOpen = $state(false);

  async function choose(row: PickerRow) {
    if (busy) return;
    busy = true;
    try {
      await pickPlan(row);
    } finally {
      busy = false;
    }
  }
</script>

{#snippet rowOf(row: PickerRow)}
  <li>
    <button type="button" class="plan-pick-row" disabled={busy} onclick={() => void choose(row)}>
      <span class="plan-pick-goal">{row.plan.goal}</span>
      <span class="plan-pick-meta">
        <span class={`plan-pick-status tone-${row.tone}`}>{row.status}</span>
        <span>{row.progress}</span>
        {#if row.needs}<span class="plan-pick-needs">{row.needs}</span>{/if}
        <span>{relTime(row.plan.updatedAt, clock.now)}</span>
      </span>
      {#if row.elsewhere}
        <span class="plan-pick-move">In “{chatTitle(row.elsewhere)}”; it moves to this chat</span>
      {/if}
    </button>
  </li>
{/snippet}

<section class="plan-pick" aria-label="Continue a plan">
  <header class="plan-pick-head">
    {#if onBack}
      <button type="button" class="task-link plan-pick-back" onclick={onBack}>
        <Icon icon={ChevronLeft} size={12} strokeWidth={2} />Back to the plan
      </button>
    {/if}
    <h3 class="plan-view-heading">Continue a plan</h3>
  </header>
  {#if picker.open.length}
    <ul class="plan-pick-list">
      {#each picker.open as row (row.plan.planId)}{@render rowOf(row)}{/each}
    </ul>
  {:else}
    <p class="task-note">No other plan is in progress in this project.</p>
  {/if}
  {#if picker.settled.length}
    <Disclosure bind:open={settledOpen} class="task-notes" summaryClass="task-notes-summary">
      {#snippet summary()}<span class="plan-view-heading">Done and abandoned · {picker.settled.length}</span>{/snippet}
      <ul class="plan-pick-list">
        {#each picker.settled as row (row.plan.planId)}{@render rowOf(row)}{/each}
      </ul>
    </Disclosure>
  {/if}
</section>

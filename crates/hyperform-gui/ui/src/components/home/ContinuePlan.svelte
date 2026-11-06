<script lang="ts">
  import type { ContinuePlanCard } from "$lib/domain/work/homeCard";
  import { openWorkPlan } from "$lib/stores/plan-actions";
  import { Button } from "$lib/ui";
  import Icon, { Workflow } from "$lib/ui/icons";
  import { relTime } from "$lib/util";

  /**
   * The project's most recent plan that is building, paused or waiting on
   * a plan change: its goal, progress and what needs you. Continue opens
   * the chat it was last in, or a new chat that takes it and goes on.
   */
  type Props = { card: ContinuePlanCard; now: number };
  let { card, now }: Props = $props();

  let busy = $state(false);

  async function go() {
    if (busy) return;
    busy = true;
    try {
      await openWorkPlan(card.plan, true);
    } finally {
      busy = false;
    }
  }
</script>

<div class="continue-plan">
  <div class="continue-plan-head">
    <Icon icon={Workflow} size={13} />
    <span class={`continue-plan-status tone-${card.tone}`}>{card.status}</span>
    {#if card.needs}<span class="continue-plan-needs">{card.needs}</span>{/if}
    <span class="continue-plan-time">{relTime(card.plan.updatedAt, now)}</span>
  </div>
  <p class="continue-plan-goal" title={card.plan.goal}>{card.plan.goal}</p>
  <div class="continue-plan-foot">
    <span class="plan-head-bar" aria-hidden="true"><span style:scale={`${card.fraction} 1`}></span></span>
    <span class="continue-plan-progress">{card.progress}</span>
    <Button size="s" variant="primary" disabled={busy} onclick={() => void go()}>Continue</Button>
  </div>
</div>

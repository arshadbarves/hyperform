<script lang="ts">
  import { agentActivity } from "$lib/domain/agentTree";
  import type { SessionView } from "$lib/domain/sessionView";
  import { laneCriteria, laneStep, laneStopLine, nowLanes, type NowLane } from "$lib/domain/work/now";
  import { ui } from "$lib/stores/ui.svelte";
  import Icon from "$lib/ui/icons";
  import StatusChip from "../agents/StatusChip.svelte";
  import PetSprite from "../pet/PetSprite.svelte";
  import { TASK_GLYPH } from "./taskGlyph";

  /**
   * What is being worked on now while the plan builds (or, replanning, the
   * ready tasks that keep running): the main agent's task first, then one
   * lane per task-bound helper with its status, step and worktree. Paused
   * (`stopping`), each lane says how it winds down instead. A helper's tag
   * opens its row in the Agents tab, a title the task's detail. Nothing
   * shows while nothing runs.
   */
  type Props = { view: SessionView; stopping?: boolean; onSelect: (taskId: string) => void };
  let { view, stopping = false, onSelect }: Props = $props();

  const lanes = $derived(nowLanes(view.work, view.sessionId, view.agents));

  function owner(lane: NowLane): string {
    if (lane.owner === "main") return "main";
    if (lane.owner === "other") return "another chat";
    if (lane.owner === "none") return "checking";
    return lane.agent?.agentType ?? "helper";
  }
</script>

{#if lanes.length}
  <section class="plan-now" class:is-stopping={stopping} aria-label={stopping ? "Stopping" : "Now"}>
    <h3 class="plan-view-heading">{stopping ? "Stopping" : "Now"}</h3>
    {#each lanes as lane (lane.task.id)}
      {@const step = stopping ? laneStopLine(lane) : laneStep(lane, lane.agentId ? agentActivity(view.tools, lane.agentId) : null)}
      {@const criteria = stopping ? null : laneCriteria(lane)}
      {@const verifying = lane.task.status.type === "verifying"}
      <div class={`plan-now-lane tone-${stopping ? "quiet" : "working"}`}>
        <div class="plan-now-head">
          <span class="task-row-glyph" class:is-live={!verifying && !stopping} aria-hidden="true">
            <Icon icon={TASK_GLYPH[verifying ? "verifying" : "active"]} size={13} />
          </span>
          <button type="button" class="plan-now-title" title="Open the task" onclick={() => onSelect(lane.task.id)}>
            {lane.task.title}
          </button>
          {#if lane.owner === "helper" && lane.agentId}
            {@const agentId = lane.agentId}
            <button
              type="button"
              class="plan-now-owner is-link"
              title="Open this helper in the Agents tab"
              onclick={() => ui.openPanel("agents", agentId)}
            >
              {#if lane.agent}<PetSprite status={lane.agent.status} />{/if}
              <span>{owner(lane)}{lane.worktree ? " · worktree" : ""}</span>
            </button>
            {#if lane.agent}<StatusChip status={lane.agent.status} />{/if}
          {:else}
            <span class="plan-now-owner">{owner(lane)}</span>
          {/if}
        </div>
        {#if step}<p class="plan-now-step">{step}</p>{/if}
        {#if criteria}<p class="plan-now-step">{criteria}</p>{/if}
      </div>
    {/each}
  </section>
{/if}

<script lang="ts">
  import { plural } from "$lib/domain/format";
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import type { WorktreeInfo } from "$lib/protocol/WorktreeInfo";
  import { Disclosure, Pill, type PillTone } from "$lib/ui";
  import Icon, { ArrowRight, ListChecks } from "$lib/ui/icons";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";
  import PetSprite from "../pet/PetSprite.svelte";
  import StatusChip from "./StatusChip.svelte";

  /**
   * One helper as one line: its sprite, type, task, status and time, and
   * Open; model, usage and its result fold away. A helper sent for a plan
   * task names that task under it. The transcript shows it as a quiet card
   * (`line`), the agents panel as a row in a tree (`row`).
   */
  type Props = {
    density: "line" | "row";
    type: string;
    description: string;
    status: AgentStatus;
    time: string | null;
    background: boolean;
    model: string | null;
    tokens: number;
    costUsd: number;
    toolCalls: number;
    worktree: WorktreeInfo | null;
    note: string | null;
    noteIsError: boolean;
    depth?: number;
    /** What it is doing right now, while it runs (panel rows). */
    activity?: string | null;
    /** The plan task it was sent for, which `onTask` opens in the Plan tab. */
    task?: string | null;
    onTask?: () => void;
    onOpen?: () => void;
  };

  let {
    density,
    type,
    description,
    status,
    time,
    background,
    model,
    tokens,
    costUsd,
    toolCalls,
    worktree,
    note,
    noteIsError,
    depth = 0,
    activity = null,
    task = null,
    onTask,
    onOpen,
  }: Props = $props();

  const WORKTREE_TONE: Record<WorktreeInfo["state"], PillTone> = {
    pending: "attention",
    conflicted: "attention",
    applied: "ok",
    discarded: "neutral",
    empty: "neutral",
  };
  const hasDetail = $derived(Boolean(note || model || tokens > 0 || worktree));
  let open = $state(false);
</script>

<div class={`agent-summary is-${density} status-${status}`} style:--depth={depth}>
  <Disclosure bind:open disabled={!hasDetail} chevron={hasDetail} summaryClass="agent-summary-head">
    {#snippet summary()}
      <PetSprite {status} />
      <span class="agent-summary-type">{type}</span>
      <span class="agent-summary-desc" title={description}>{description}</span>
      {#if background && density === "line"}<Pill>background</Pill>{/if}
      <StatusChip {status} />
      {#if time}<span class="agent-summary-time">{time}</span>{/if}
    {/snippet}
    {#snippet actions()}
      {#if onOpen}
        <button type="button" class="agent-summary-open" aria-label={`Open the ${type} transcript`} onclick={onOpen}>
          {#if density === "line"}<span>Open</span>{/if}
          <Icon icon={ArrowRight} size={11} />
        </button>
      {/if}
    {/snippet}
    <div class="agent-summary-detail">
      <p class="agent-summary-meta">
        {#if model}<span>{shortModel(model)}</span>{/if}
        {#if tokens > 0}<span>{fmtTokens(tokens)} tokens</span>{/if}
        {#if costUsd > 0}<span>{fmtCost(costUsd)}</span>{/if}
        {#if toolCalls > 0}<span>{plural(toolCalls, "tool call")}</span>{/if}
        {#if background && density === "row"}<Pill>background</Pill>{/if}
        {#if worktree}<Pill tone={WORKTREE_TONE[worktree.state]}>worktree · {worktree.state}</Pill>{/if}
      </p>
      {#if note}<p class={`agent-summary-note${noteIsError ? " is-error" : ""}`}>{note}</p>{/if}
    </div>
  </Disclosure>
  {#if task}
    <button type="button" class="agent-summary-task" title="Open the task in the Plan tab" disabled={!onTask} onclick={onTask}>
      <Icon icon={ListChecks} size={11} />
      <span>{task}</span>
    </button>
  {/if}
  {#if activity}<p class="agent-summary-now">{activity}</p>{/if}
</div>

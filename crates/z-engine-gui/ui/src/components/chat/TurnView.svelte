<script lang="ts">
  import { untrack } from "svelte";
  import { turnFiles } from "$lib/domain/receipt";
  import type { StreamingMessage, ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo } from "$lib/domain/timeline/blocks";
  import { answerText, groupTurnItems, splitTrailingCompactions, workSection } from "$lib/domain/timeline/groups";
  import type { TimelineTurn } from "$lib/domain/timeline/turns";
  import { tasksDoneIn } from "$lib/domain/work/progress";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { whenVisible } from "$lib/ui/whenVisible";
  import AssistantText from "./AssistantText.svelte";
  import ThinkingDisclosure from "./ThinkingDisclosure.svelte";
  import TurnActions from "./TurnActions.svelte";
  import TurnBlocks from "./TurnBlocks.svelte";
  import TurnFooter from "./TurnFooter.svelte";
  import UserCard from "./UserCard.svelte";
  import WorkSummary from "./WorkSummary.svelte";

  /**
   * One turn: your prompt, the agent's work and its answer. Once the turn is
   * done and made two or more tool calls, the work folds into one line above
   * the answer, and the turn's actions and receipt follow the answer.
   * A `lazy` turn is an empty placeholder until it nears the screen.
   */
  type Props = {
    turn: TimelineTurn;
    results: Record<string, ToolResultInfo>;
    tools: Record<string, ToolCallView>;
    agents: Record<string, AgentInfo>;
    agentLinks: Record<string, string>;
    checks: CheckRecord[];
    projectRoot: string | null;
    live?: StreamingMessage[];
    lazy?: boolean;
    canRestoreCode?: (messageId: string) => boolean;
    /** When the attached plan's done tasks finished (`doneTimes`), to count a turn's. */
    tasksDoneAt?: readonly number[];
    onRewind?: (message: Message, scope: RewindScope) => void;
    onOpenAgent: (agentId: string) => void;
  };
  let {
    turn,
    results,
    tools,
    agents,
    agentLinks,
    checks,
    projectRoot,
    live = [],
    lazy = false,
    canRestoreCode,
    tasksDoneAt = [],
    onRewind,
    onOpenAgent,
  }: Props = $props();

  let root: HTMLElement | undefined = $state();
  let near = $state(untrack(() => !lazy));
  let open = $state(false);

  $effect(() => {
    if (near || !root) return;
    return whenVisible(root, () => (near = true));
  });

  const split = $derived(splitTrailingCompactions(turn.items));
  const section = $derived(workSection(split.body, !turn.active, (callId) => results[callId]?.isError === true));
  const done = $derived(!turn.active && live.length === 0);
  const answer = $derived(done ? answerText(section) : "");
  const durationMs = $derived(turn.record ? turn.record.finishedAt - turn.record.startedAt : null);
  const tasksDone = $derived(turn.record ? tasksDoneIn(tasksDoneAt, turn.record.startedAt, turn.record.finishedAt) : 0);
  const shared = $derived({ results, tools, agents, agentLinks, projectRoot, busy: turn.active, onOpenAgent });
  const files = $derived(
    turn.record
      ? turnFiles(
          turn.items.flatMap((item) =>
            item.kind === "tool"
              ? [{ name: item.use.name, input: item.use.input, ok: results[item.use.callId]?.isError !== true }]
              : [],
          ),
          projectRoot,
        )
      : [],
  );
</script>

<section class="turn" class:is-live={turn.active} class:is-pending={!near} data-turn={turn.key} bind:this={root}>
  {#if near}
    {#if turn.user}
      <UserCard message={turn.user} />
    {/if}

    {#if split.body.length > 0 || live.length > 0}
      <div class="assistant-turn">
        {#if section.fold}
          <div class="work">
            <WorkSummary uses={section.uses} {results} {tools} {durationMs} {tasksDone} bind:open />
            {#if open || section.pinned.length > 0}
              <div class="work-body" class:is-open={open}>
                <TurnBlocks blocks={groupTurnItems(open ? section.work : section.pinned)} {...shared} />
              </div>
            {/if}
          </div>
          <TurnBlocks blocks={groupTurnItems(section.answer)} {...shared} />
        {:else}
          <TurnBlocks blocks={groupTurnItems(split.body)} {...shared} />
        {/if}
        {#each live as stream (stream.messageId)}
          {#if stream.thinking}
            <ThinkingDisclosure text={stream.thinking} streaming={!stream.text} />
          {/if}
          {#if stream.text}
            <AssistantText text={stream.text} streaming />
          {/if}
        {/each}
      </div>
    {/if}

    {#if done}
      <div class="turn-end">
        <TurnActions
          {answer}
          changed={files.length > 0}
          prompt={turn.user}
          canRestoreCode={turn.user ? (canRestoreCode?.(turn.user.id) ?? false) : false}
          {onRewind}
        />
        {#if turn.record}<TurnFooter record={turn.record} {checks} {files} />{/if}
      </div>
    {/if}

    {#if split.after.length > 0}
      <TurnBlocks blocks={groupTurnItems(split.after)} {...shared} />
    {/if}
  {/if}
</section>

<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { SessionView } from "$lib/domain/sessionView";
  import { pairResults, toolUses, visibleText } from "$lib/domain/timeline/blocks";
  import { linkAgentCalls, liveStreams } from "$lib/domain/timeline/toolState";
  import { buildTimeline, type TimelineTurn } from "$lib/domain/timeline/turns";
  import { followTotal, hasOlder, openWindow, reveal, sampleEvenly, showOlder, WINDOW_EAGER } from "$lib/domain/timeline/window";
  import { doneTimes } from "$lib/domain/work/progress";
  import { movedTraces, planTraces, restoreTimes } from "$lib/domain/work/trace";
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { cancelTurn, rewind, sessions } from "$lib/runtime";
  import { composer } from "$lib/stores/composer.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { scrollParent } from "$lib/ui/whenVisible";
  import { agentLabel } from "../planning/PendingInteractions.svelte";
  import PlanReady from "../planning/PlanReady.svelte";
  import Suggestions from "../planning/Suggestions.svelte";
  import ChatTimeline from "./ChatTimeline.svelte";
  import TrustBanner from "./TrustBanner.svelte";
  import TurnView from "./TurnView.svelte";
  import { anchorTurns, isAtBottom, observeTop } from "./transcriptScroll";

  /**
   * The chat's turns in one column. A long chat renders a window of its newest
   * turns and reaches further back as you scroll up (see `timeline/window.ts`);
   * turns outside the newest few fill in as they near the screen.
   */
  const NO_LIST: never[] = [];
  const NO_RECORD: Record<string, never> = {};
  const RAIL_DOTS = 40;

  const view: SessionView | null = $derived(sessions.active);
  const messages = $derived(view?.messages ?? NO_LIST);
  const turns = $derived(view?.turns ?? NO_LIST);
  const turnStarts = $derived(view?.turnStarts ?? NO_RECORD);
  const steeringIds = $derived(view?.steeringIds ?? NO_RECORD);
  const rawOutputs = $derived(view?.outputs ?? NO_LIST);
  const outputs = $derived(rawOutputs.filter((o) => o.name !== "shell"));
  const errors = $derived(view?.errors ?? NO_LIST);
  const routes = $derived(view?.routes ?? NO_LIST);
  const compactions = $derived(view?.compactions ?? NO_LIST);
  const taskViews = $derived(view?.taskViews ?? NO_LIST);
  const activeMessageId = $derived(view?.activeTurn?.messageId ?? null);
  const busy = $derived((view?.status ?? "idle") !== "idle");
  const streaming = $derived(view?.streaming ?? NO_RECORD);
  const tools = $derived(view?.tools ?? NO_RECORD);
  const agents = $derived(view?.agents ?? NO_RECORD);
  const checks = $derived(view?.checks ?? NO_LIST);
  const checkpoints = $derived(view?.checkpoints ?? NO_LIST);
  const projectRoot = $derived(view?.info?.projectRoot ?? null);
  const plans = $derived(view ? Object.values(view.plans) : NO_LIST);
  const work = $derived(view?.work ?? null);
  const notices = $derived(view?.notices ?? NO_LIST);
  const traces = $derived([...(work ? planTraces(work) : NO_LIST), ...movedTraces(notices)]);
  const restores = $derived(view ? restoreTimes(view.packets) : NO_LIST);
  const tasksDoneAt = $derived(work ? doneTimes(work.tasks) : NO_LIST);

  const timeline: TimelineTurn[] = $derived(
    buildTimeline({
      messages,
      turns,
      turnStarts,
      steeringIds,
      activeMessageId,
      busy,
      outputs,
      errors,
      routes,
      compactions,
      taskViews,
      traces,
      restores,
    }),
  );
  const results = $derived(pairResults(messages));
  const agentLinks = $derived(linkAgentCalls(messages.flatMap(toolUses), Object.values(agents)));
  const live = $derived(liveStreams(streaming));
  const checkpointIds = $derived(new Set(checkpoints.map((c) => c.messageId)));
  const rail = $derived(
    sampleEvenly(
      timeline.flatMap((turn) => (turn.user ? [turn.user] : [])),
      RAIL_DOTS,
    ).map((message) => ({ id: message.id, text: visibleText(message) })),
  );
  const liveTurn: TimelineTurn = { key: "live", user: null, items: [], record: null, active: true };

  let win = $state(untrack(() => openWindow(timeline.length)));
  let windowFor = untrack(() => view?.sessionId ?? null);
  const first = $derived(Math.min(win.start, timeline.length));
  const shown = $derived(first === 0 ? timeline : timeline.slice(first));

  let inner: HTMLDivElement | undefined = $state();
  let marker: HTMLDivElement | undefined = $state();
  let scroller: HTMLElement | null = $state(null);
  let loading = false;

  $effect.pre(() => {
    const id = view?.sessionId ?? null;
    const total = timeline.length;
    untrack(() => {
      if (id !== windowFor) {
        windowFor = id;
        win = openWindow(total);
      } else {
        win = followTotal(win, total, !scroller || isAtBottom(scroller));
      }
    });
  });

  $effect(() => {
    if (!inner) return;
    const found = scrollParent(inner);
    scroller = found;
    return found ? anchorTurns(inner, found) : undefined;
  });

  $effect(() => {
    if (!marker || !scroller) return;
    return observeTop(marker, scroller, () => void loadOlder());
  });

  async function loadOlder() {
    const el = scroller;
    if (loading || !el || !hasOlder(win)) return;
    loading = true;
    const fromBottom = el.scrollHeight - el.scrollTop;
    win = showOlder(win);
    await tick();
    el.scrollTop = el.scrollHeight - fromBottom;
    loading = false;
  }

  async function jumpTo(messageId: string) {
    const index = timeline.findIndex((turn) => turn.user?.id === messageId);
    if (index < 0) return;
    win = reveal(win, index);
    await tick();
    // An instant jump: turns filling in along a smooth scroll would re-anchor it midway.
    inner?.querySelector(`[data-turn="${CSS.escape(timeline[index].key)}"]`)?.scrollIntoView({ block: "start" });
  }

  async function onRewind(message: Message, scope: RewindScope) {
    if (busy) await cancelTurn();
    if ((await rewind(message.id, scope)) && scope !== "code") composer.setDraft(visibleText(message));
  }

  function openAgent(agentId: string) {
    ui.openPanel("agents", agentId);
  }
</script>

<div class="transcript-stage">
  <div class="transcript-inner" bind:this={inner}>
    {#if hasOlder(win)}
      <div class="transcript-older" bind:this={marker} aria-hidden="true"></div>
    {/if}
    {#each shown as turn, i (turn.key)}
      <TurnView
        {turn}
        {results}
        {tools}
        {agents}
        {agentLinks}
        {checks}
        {projectRoot}
        live={first + i === timeline.length - 1 ? live : undefined}
        lazy={first + i < timeline.length - WINDOW_EAGER}
        canRestoreCode={(id) => checkpointIds.has(id)}
        {tasksDoneAt}
        {onRewind}
        onOpenAgent={openAgent}
      />
    {/each}
    {#if timeline.length === 0 && live.length > 0}
      <TurnView turn={liveTurn} {results} {tools} {agents} {agentLinks} {checks} {projectRoot} {live} onOpenAgent={openAgent} />
    {/if}
    {#if view?.compacting}
      <div class="compaction-live" role="status">
        <span class="compaction-rule" aria-hidden="true"></span>
        <span class="compaction-live-label">Compacting context…</span>
        <span class="compaction-rule" aria-hidden="true"></span>
      </div>
    {/if}
    {#if view}
      {#each plans as pending (pending.requestId)}
        <PlanReady agentLabel={agentLabel(view, pending.agentId)} />
      {/each}
      <Suggestions {view} />
      {#if view.trustRequest}<TrustBanner request={view.trustRequest} />{/if}
    {/if}
  </div>
  <ChatTimeline prompts={rail} onJump={(id) => void jumpTo(id)} />
</div>

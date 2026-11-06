<script lang="ts">
  import { untrack } from "svelte";
  import { agentTranscript } from "$lib/commands";
  import { isAgentDone } from "$lib/domain/agentTree";
  import type { SessionView } from "$lib/domain/sessionView";
  import { pairResults, toolUses } from "$lib/domain/timeline/blocks";
  import { linkAgentCalls, liveStreams } from "$lib/domain/timeline/toolState";
  import { buildTimeline } from "$lib/domain/timeline/turns";
  import { usageLine } from "$lib/domain/usage";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { Message } from "$lib/protocol/Message";
  import { errorText } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { RefreshCw } from "$lib/ui/icons";
  import { fmtCost, shortModel } from "$lib/util";
  import TurnView from "../chat/TurnView.svelte";
  import TodoChecklist from "../planning/TodoChecklist.svelte";
  import StatusChip from "./StatusChip.svelte";

  type Props = { view: SessionView; agent: AgentInfo; onOpenAgent: (agentId: string) => void };
  let { view, agent, onOpenAgent }: Props = $props();

  let messages = $state<Message[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let loadedFor = $state<string | null>(null);

  const running = $derived(!isAgentDone(agent.status));
  const refreshKey = $derived(`${agent.agentId}:${agent.status}:${agent.toolCalls}`);
  const timeline = $derived(buildTimeline({ messages, busy: running }));
  const results = $derived(pairResults(messages));
  const agentLinks = $derived(linkAgentCalls(messages.flatMap(toolUses), Object.values(view.agents), agent.agentId));
  const live = $derived(liveStreams(view.streaming, agent.agentId));
  const todos = $derived(view.todos[agent.agentId] ?? []);

  async function load() {
    loading = true;
    const id = agent.agentId;
    try {
      const next = await agentTranscript(view.sessionId, id);
      if (id !== agent.agentId) return;
      messages = next;
      error = null;
      loadedFor = id;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void refreshKey;
    const delay = untrack(() => (loadedFor === agent.agentId ? 500 : 0));
    const timer = setTimeout(() => void load(), delay);
    return () => clearTimeout(timer);
  });
</script>

<div class="agent-transcript">
  <div class="agent-transcript-head">
    <StatusChip status={agent.status} />
    <span class="agent-transcript-meta">
      {shortModel(agent.model)}{usageLine(agent.usage) ? ` · ${usageLine(agent.usage)}` : ""}{agent.costUsd > 0
        ? ` · ${fmtCost(agent.costUsd)}`
        : ""}
    </span>
    <Button variant="icon" spinning={loading} title="Reload transcript" aria-label="Reload transcript" onclick={() => void load()}>
      <Icon icon={RefreshCw} size={13} />
    </Button>
  </div>

  {#if todos.length > 0}
    <section class="work-section agent-transcript-plan"><TodoChecklist {todos} /></section>
  {/if}
  {#if agent.error}<p class="agent-row-note is-error">{agent.error}</p>{/if}
  {#if error}<p class="work-empty">Could not load the transcript: {error}</p>{/if}

  <div class="agent-transcript-body">
    {#each timeline as turn, i (turn.key)}
      <TurnView
        {turn}
        {results}
        tools={view.tools}
        agents={view.agents}
        {agentLinks}
        checks={view.checks}
        projectRoot={view.info?.projectRoot ?? null}
        live={i === timeline.length - 1 ? live : undefined}
        {onOpenAgent}
      />
    {/each}
    {#if timeline.length === 0 && !loading && !error}
      <p class="work-empty">{running ? "Waiting for the agent's first message…" : "No messages recorded."}</p>
    {/if}
  </div>
</div>

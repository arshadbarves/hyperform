<script lang="ts">
  import type { ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo, ToolUseRef } from "$lib/domain/timeline/blocks";
  import { runLine, summarizeRun } from "$lib/domain/timeline/groups";
  import { resolveToolCall } from "$lib/domain/timeline/toolState";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import { Disclosure } from "$lib/ui";
  import ToolCall from "./ToolCall.svelte";

  /**
   * A run of tool calls as one line: what was done, the live step, and any
   * failure. Unfolding shows each call's own card (itself folded).
   */
  type Props = {
    uses: ToolUseRef[];
    results: Record<string, ToolResultInfo>;
    tools: Record<string, ToolCallView>;
    busy: boolean;
    projectRoot: string | null;
    agents: Record<string, AgentInfo>;
    agentLinks: Record<string, string>;
    onOpenAgent: (agentId: string) => void;
  };
  let { uses, results, tools, busy, projectRoot, agents, agentLinks, onOpenAgent }: Props = $props();

  const TAIL_LINES = 3;
  let open = $state(false);
  const calls = $derived(uses.map((use) => resolveToolCall(use, results[use.callId], tools[use.callId], busy)));
  const run = $derived(summarizeRun(calls));
  const liveBash = $derived(calls.findLast((c) => c.status === "running" && c.name === "Bash" && c.progress));
  const failure = $derived(calls.findLast((c) => c.status === "error"));

  function tail(text: string): string {
    const lines = text.replace(/\n+$/, "").split("\n");
    return lines.slice(-TAIL_LINES).join("\n");
  }
</script>

<div class={`tool-group is-${run.state}`}>
  <Disclosure bind:open summaryClass="tool-group-summary" label={`${runLine(run)}. Show each step`}>
    {#snippet summary()}
      <span class="tool-group-dot" aria-hidden="true"></span>
      <span class="tool-group-line">{runLine(run)}</span>
      {#if run.current}<span class="tool-group-now">{run.current}</span>{/if}
      {#if run.failed}<span class="tool-group-flag is-failed">{run.failed} failed</span>{/if}
      {#if run.denied}<span class="tool-group-flag is-denied">{run.denied} denied</span>{/if}
      <span class="tool-group-count">{calls.length} steps</span>
    {/snippet}
    <div class="tool-group-body">
      {#each uses as use (use.callId)}
        {@const agentId = agentLinks[use.callId]}
        <ToolCall
          toolUse={use}
          result={results[use.callId]}
          live={tools[use.callId]}
          {busy}
          {projectRoot}
          agent={agentId ? (agents[agentId] ?? null) : null}
          {onOpenAgent}
        />
      {/each}
    </div>
  </Disclosure>
  {#if !open && liveBash}
    <pre class="tool-peek is-live">{tail(liveBash.progress)}</pre>
  {:else if !open && failure && (failure.output || failure.progress)}
    <pre class="tool-peek is-failed">{tail(failure.output || failure.progress)}</pre>
  {/if}
</div>

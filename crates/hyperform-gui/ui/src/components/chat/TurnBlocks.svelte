<script lang="ts">
  import type { ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo } from "$lib/domain/timeline/blocks";
  import type { TurnBlock } from "$lib/domain/timeline/groups";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import AssistantText from "./AssistantText.svelte";
  import LocalCards from "./LocalCards.svelte";
  import ThinkingDisclosure from "./ThinkingDisclosure.svelte";
  import ToolCall from "./tools/ToolCall.svelte";
  import ToolGroup from "./tools/ToolGroup.svelte";

  /** A list of transcript blocks: prose, thinking, tool calls and runs, and local cards. */
  type Props = {
    blocks: TurnBlock[];
    results: Record<string, ToolResultInfo>;
    tools: Record<string, ToolCallView>;
    agents: Record<string, AgentInfo>;
    agentLinks: Record<string, string>;
    projectRoot: string | null;
    busy: boolean;
    onOpenAgent: (agentId: string) => void;
  };
  let { blocks, results, tools, agents, agentLinks, projectRoot, busy, onOpenAgent }: Props = $props();
</script>

{#each blocks as block (block.key)}
  {#if block.kind === "run"}
    <ToolGroup uses={block.uses} {results} {tools} {busy} {projectRoot} {agents} {agentLinks} {onOpenAgent} />
  {:else if block.item.kind === "text"}
    <AssistantText text={block.item.text} />
  {:else if block.item.kind === "thinking"}
    <ThinkingDisclosure text={block.item.text} redacted={block.item.redacted} />
  {:else if block.item.kind === "tool"}
    {@const use = block.item.use}
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
  {:else}
    <LocalCards item={block.item} />
  {/if}
{/each}

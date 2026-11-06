<script lang="ts">
  import type { ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo, ToolUseRef } from "$lib/domain/timeline/blocks";
  import { resolveToolCall } from "$lib/domain/timeline/toolState";
  import { toolMeta } from "$lib/domain/tools/toolMeta";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import AgentToolCard from "./AgentToolCard.svelte";
  import BashToolCard from "./BashToolCard.svelte";
  import EditToolCard from "./EditToolCard.svelte";
  import GenericToolCard from "./GenericToolCard.svelte";
  import TodoToolCard from "./TodoToolCard.svelte";
  import WebToolCard from "./WebToolCard.svelte";

  type Props = {
    toolUse: ToolUseRef;
    result: ToolResultInfo | undefined;
    live: ToolCallView | undefined;
    busy: boolean;
    projectRoot: string | null;
    agent?: AgentInfo | null;
    onOpenAgent?: (agentId: string) => void;
  };
  let { toolUse, result, live, busy, projectRoot, agent = null, onOpenAgent }: Props = $props();

  const call = $derived(resolveToolCall(toolUse, result, live, busy));
  const meta = $derived(toolMeta(toolUse.name));
</script>

{#if meta.family === "edit" || meta.family === "write"}
  <EditToolCard {call} {projectRoot} />
{:else if meta.family === "bash"}
  <BashToolCard {call} />
{:else if meta.family === "agent"}
  <AgentToolCard {call} {agent} onOpen={onOpenAgent} />
{:else if meta.family === "todo"}
  <TodoToolCard {call} />
{:else if meta.family === "web"}
  <WebToolCard {call} />
{:else}
  <GenericToolCard {call} {meta} {projectRoot} />
{/if}

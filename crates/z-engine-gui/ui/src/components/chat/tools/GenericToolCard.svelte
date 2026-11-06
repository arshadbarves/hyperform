<script lang="ts">
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { prettyJson, relPath } from "$lib/domain/tools/toolInput";
  import { toolSubject, type ToolFamily, type ToolMeta } from "$lib/domain/tools/toolMeta";
  import {
    FileText,
    HelpCircle,
    ListChecks,
    Plug,
    Search,
    SquareTerminal,
    Wrench,
    type IconSvgElement,
  } from "$lib/ui/icons";
  import ToolFrame from "./ToolFrame.svelte";
  import ToolOutput from "./ToolOutput.svelte";

  type Props = { call: ToolCallState; meta: ToolMeta; projectRoot: string | null };
  let { call, meta, projectRoot }: Props = $props();

  const ICONS: Partial<Record<ToolFamily, IconSvgElement>> = {
    read: FileText,
    search: Search,
    job: SquareTerminal,
    question: HelpCircle,
    plan: ListChecks,
    mcp: Plug,
  };
  const LABELS: Partial<Record<ToolFamily, string>> = { question: "Asked", plan: "Plan" };

  const showsInput = $derived(meta.family === "mcp" || meta.family === "generic");
  const input = $derived(showsInput ? prettyJson(call.input) : "");
  const subject = $derived(relPath(toolSubject(call.name, call.input), projectRoot));
  let open = $state(false);
</script>

<ToolFrame
  {call}
  icon={ICONS[meta.family] ?? Wrench}
  label={LABELS[meta.family] ?? meta.label}
  badge={meta.server}
  {subject}
  extra={call.summary || undefined}
  expandable={Boolean(call.output || call.progress || (input && input !== "{}"))}
  bind:open
>
  {#if input && input !== "{}"}
    <ToolOutput text={input} label="Input" />
  {/if}
  {#if call.status === "running" && call.progress}
    <ToolOutput text={call.progress} live />
  {:else}
    <ToolOutput text={call.output} label={call.status === "error" ? "Error" : "Result"} />
  {/if}
</ToolFrame>

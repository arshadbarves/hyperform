<script lang="ts">
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { bool, firstLine, str } from "$lib/domain/tools/toolInput";
  import { SquareTerminal } from "$lib/ui/icons";
  import ToolFrame from "./ToolFrame.svelte";
  import ToolOutput from "./ToolOutput.svelte";

  type Props = { call: ToolCallState };
  let { call }: Props = $props();

  const command = $derived(str(call.input, "command"));
  const description = $derived(str(call.input, "description"));
  const background = $derived(bool(call.input, "run_in_background"));
  const running = $derived(call.status === "running");
  const liveText = $derived(running ? call.progress : "");
  const finalText = $derived(running ? "" : call.output || call.progress);
  const tail = $derived(liveText.replace(/\n+$/, "").split("\n").slice(-3).join("\n"));
  let toggled = $state<boolean | null>(null);
  const open = $derived(toggled ?? call.status === "error");
</script>

{#snippet peek()}
  {#if running && tail}<pre class="tool-peek is-live">{tail}</pre>{/if}
{/snippet}

<ToolFrame
  {call}
  icon={SquareTerminal}
  label="Bash"
  subject={firstLine(command) || call.title}
  badge={background ? "background" : undefined}
  extra={!running && call.summary ? call.summary : undefined}
  expandable={Boolean(command || call.output || call.progress)}
  bind:open={() => open, (value) => (toggled = value)}
  {peek}
>
  {#if description}
    <p class="tool-note">{description}</p>
  {/if}
  <pre class="tool-command"><span class="tool-command-prompt">$</span> {command}</pre>
  {#if liveText}
    <ToolOutput text={liveText} live />
  {:else if finalText}
    <ToolOutput text={finalText} label={call.status === "error" ? "Failed" : "Output"} />
  {/if}
</ToolFrame>

<script lang="ts">
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { str } from "$lib/domain/tools/toolInput";
  import { Globe, Search } from "$lib/ui/icons";
  import ToolFrame from "./ToolFrame.svelte";
  import ToolOutput from "./ToolOutput.svelte";

  type Props = { call: ToolCallState };
  let { call }: Props = $props();

  const isFetch = $derived(call.name === "WebFetch");
  const url = $derived(str(call.input, "url"));
  const query = $derived(str(call.input, "query"));
  const prompt = $derived(str(call.input, "prompt"));
  let open = $state(false);
</script>

<ToolFrame
  {call}
  icon={isFetch ? Globe : Search}
  label={call.name}
  subject={isFetch ? url : query}
  extra={call.summary || undefined}
  expandable={Boolean(call.output || prompt)}
  bind:open
>
  {#if isFetch && url}
    <p class="tool-note"><span class="tool-note-key">URL</span> {url}</p>
  {/if}
  {#if prompt}
    <p class="tool-note"><span class="tool-note-key">Asked</span> {prompt}</p>
  {/if}
  <ToolOutput text={call.output} label={isFetch ? "Extract" : "Results"} />
</ToolFrame>

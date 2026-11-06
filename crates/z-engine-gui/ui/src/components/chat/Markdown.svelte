<script lang="ts">
  import { setContext } from "svelte";
  import { Renderer } from "svelte-exmarkdown";
  import { markdownChunks } from "$lib/domain/timeline/markdownChunks";
  import { parseMarkdown } from "$lib/markdown";
  import CodeBlock, { STREAMING } from "./CodeBlock.svelte";

  /**
   * Markdown rendered block by block. Finished blocks come from the parse
   * cache; while a reply streams, only its last block is parsed again.
   */
  type Props = { text: string; streaming?: boolean };
  let { text, streaming = false }: Props = $props();

  // svelte-exmarkdown's Renderer looks its tag overrides up in this context.
  setContext("components", { current: { pre: CodeBlock } });
  setContext(STREAMING, () => streaming);
  const chunks = $derived(markdownChunks(text));
</script>

<!-- Streaming blocks update in place; a finished block that changes starts over, so no stale colors stay. -->
<div class="md">
  {#each chunks as chunk, i (streaming ? i : `${i}:${chunk}`)}
    <Renderer astNode={parseMarkdown(chunk, !streaming || i < chunks.length - 1)} />
  {/each}
</div>

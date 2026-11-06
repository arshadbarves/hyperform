<script lang="ts">
  import Markdown from "./Markdown.svelte";

  type Props = { text: string; streaming?: boolean };
  let { text, streaming = false }: Props = $props();
</script>

<div class={`msg assistant${streaming ? " streaming" : ""}`}>
  <Markdown {text} {streaming} />
</div>

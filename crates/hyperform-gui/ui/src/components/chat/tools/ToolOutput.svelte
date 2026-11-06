<script lang="ts">
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy } from "$lib/ui/icons";

  type Props = { text: string; live?: boolean; label?: string; tailLines?: number };
  let { text, live = false, label, tailLines = 40 }: Props = $props();

  const feedback = copyFeedback();
  let pre: HTMLPreElement | undefined = $state();
  const body = $derived(text.replace(/\n$/, ""));
  const lines = $derived(body ? body.split("\n") : []);
  const shown = $derived(live && lines.length > tailLines ? lines.slice(-tailLines).join("\n") : body);

  $effect(() => {
    void shown;
    if (live && pre) pre.scrollTop = pre.scrollHeight;
  });
</script>

{#if body}
  <div class="tool-output-wrap">
    <div class="tool-output-bar">
      <span class="tool-output-lines">
        {label ? `${label} · ` : ""}{lines.length} line{lines.length === 1 ? "" : "s"}{live ? " · live" : ""}
      </span>
      <button type="button" class="tool-copy-btn" onclick={() => void feedback.copy(text)}>
        <Icon icon={feedback.copied ? Check : Copy} size={10} />
        {feedback.copied ? "Copied" : "Copy"}
      </button>
    </div>
    <pre bind:this={pre} class={live ? "tool-tail" : "tool-full"}>{shown}</pre>
  </div>
{/if}

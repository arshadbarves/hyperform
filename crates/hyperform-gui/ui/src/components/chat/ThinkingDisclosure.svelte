<script lang="ts">
  import Icon, { ChevronRight } from "$lib/ui/icons";

  /** The model's reasoning as one quiet line; it unfolds into the full text. */
  type Props = { text: string; streaming?: boolean; redacted?: boolean };
  let { text, streaming = false, redacted = false }: Props = $props();

  let open = $state(false);
  const chars = $derived(text.length >= 1000 ? `${(text.length / 1000).toFixed(1)}k characters` : `${text.length} characters`);
  const label = $derived(redacted ? "Reasoning hidden" : streaming ? "Thinking…" : "Thought");
</script>

<div class={`msg thinking${open ? " open" : ""}${streaming ? " streaming" : ""}`}>
  <button
    type="button"
    class="thinking-head"
    disabled={redacted || !text}
    aria-expanded={redacted ? undefined : open}
    title={text ? chars : undefined}
    onclick={() => (open = !open)}
  >
    <span class="thinking-label">{label}</span>
    {#if text && !redacted}
      <span class="thinking-chevron" aria-hidden="true"><Icon icon={ChevronRight} size={11} strokeWidth={2} /></span>
    {/if}
  </button>
  {#if open && text}
    <pre class="thinking-body">{text}</pre>
  {/if}
</div>

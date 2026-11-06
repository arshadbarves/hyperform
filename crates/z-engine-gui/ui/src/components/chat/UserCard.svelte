<script lang="ts">
  import { mediaSrc, messageDocuments, messageImages, visibleText } from "$lib/domain/timeline/blocks";
  import { commandChip } from "$lib/domain/timeline/commandChip";
  import type { Message } from "$lib/protocol/Message";
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy, FileText } from "$lib/ui/icons";
  import CommandChip from "./CommandChip.svelte";

  /** Your message: a quiet bubble on the right. Copy appears beside it on hover. */
  type Props = { message: Message };
  let { message }: Props = $props();

  const COLLAPSE_CHARS = 380;
  const COLLAPSE_LINES = 6;

  const feedback = copyFeedback();
  const chip = $derived(commandChip(message));
  const text = $derived(visibleText(message));
  const images = $derived(messageImages(message));
  const documents = $derived(messageDocuments(message));
  const isLong = $derived(!chip && (text.length > COLLAPSE_CHARS || text.split("\n").length > COLLAPSE_LINES));
  let expanded = $state(false);

  async function copy() {
    if (!(await feedback.copy(text))) pushToast("Copy failed", "warn");
  }
</script>

<div class="user-message-row" id={`msg-${message.id}`} data-msg-id={message.id}>
  <div class="user-message">
    {#if text}
      <div class="user-message-actions">
        <button
          type="button"
          class="turn-action"
          class:is-copied={feedback.copied}
          title={feedback.copied ? "Copied" : "Copy"}
          aria-label="Copy your message"
          onclick={() => void copy()}
        >
          <Icon icon={feedback.copied ? Check : Copy} size={13} />
        </button>
      </div>
    {/if}
    <div class="user-message-bubble">
      {#if chip}
        <CommandChip {chip} />
      {:else if text}
        <div class={`user-prompt-text${isLong && !expanded ? " collapsed" : ""}`}>{text}</div>
      {/if}
      {#if images.length > 0}
        <div class="user-attached-images">
          {#each images as source, i (i)}
            <img src={mediaSrc(source)} alt={`attachment ${i + 1}`} class="user-img-thumb" />
          {/each}
        </div>
      {/if}
      {#if documents.length > 0}
        <div class="user-attached-docs">
          {#each documents as doc, i (i)}
            <span class="user-doc-chip"><Icon icon={FileText} size={11} />{doc.title ?? "Document"}</span>
          {/each}
        </div>
      {/if}
      {#if isLong}
        <button type="button" class="user-expand-btn" onclick={() => (expanded = !expanded)}>
          {expanded ? "Show less" : "Show more"}
        </button>
      {/if}
    </div>
  </div>
</div>

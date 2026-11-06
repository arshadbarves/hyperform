<script lang="ts">
  import { tick } from "svelte";
  import { queuePreview, queueTitle, removeQueued, replaceQueued } from "$lib/domain/queuePreview";
  import Icon, { X } from "$lib/ui/icons";

  /** Steering messages waiting for the next round boundary; click one to edit it. */
  type Props = { items: string[]; onChange: (queued: string[]) => void };
  let { items, onChange }: Props = $props();

  let editing = $state<number | null>(null);
  let draft = $state("");
  let input: HTMLInputElement | undefined = $state();

  async function edit(index: number) {
    editing = index;
    draft = items[index] ?? "";
    await tick();
    input?.focus();
    input?.select();
  }

  function commit() {
    if (editing === null) return;
    const next = replaceQueued(items, editing, draft);
    editing = null;
    if (next.join("\u0000") !== items.join("\u0000")) onChange(next);
  }
</script>

{#if items.length > 0}
  <div class="queue-strip" role="group" aria-label="Queued steering messages">
    <span class="queue-label">queued</span>
    {#each items as item, i (i)}
      {#if editing === i}
        <input
          bind:this={input}
          bind:value={draft}
          class="queue-edit-input"
          aria-label={`Edit queued message ${i + 1}`}
          onblur={commit}
          onkeydown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") editing = null;
          }}
        />
      {:else}
        <span class="queue-pill" title={queueTitle(item)}>
          <button type="button" class="queue-pill-text" onclick={() => void edit(i)}>{queuePreview(item)}</button>
          <button
            type="button"
            class="queue-pill-x"
            aria-label={`Remove queued message ${i + 1}`}
            onclick={() => onChange(removeQueued(items, i))}
          >
            <Icon icon={X} size={9} strokeWidth={2.4} />
          </button>
        </span>
      {/if}
    {/each}
  </div>
{/if}

<script lang="ts">
  /** The prompt rail at the column's edge: one dot per prompt (evenly sampled in a long chat). */
  type Props = { prompts: Array<{ id: string; text: string }>; onJump: (messageId: string) => void };
  let { prompts, onJump }: Props = $props();

  let hoveredId = $state<string | null>(null);

  function getSnippet(text: string): string {
    const clean = text.replace(/[\n\r]+/g, " ").trim();
    return clean.length > 55 ? `${clean.slice(0, 52)}…` : clean;
  }
</script>

{#if prompts.length >= 2}
  <nav class="chat-timeline-rail" aria-label="Jump to conversation prompt">
    <div class="chat-timeline-track">
      {#each prompts as prompt (prompt.id)}
        <div class="chat-timeline-node">
          <button
            type="button"
            class="chat-timeline-pill"
            aria-label={`Jump to: ${getSnippet(prompt.text)}`}
            onclick={() => onJump(prompt.id)}
            onmouseenter={() => (hoveredId = prompt.id)}
            onmouseleave={() => {
              if (hoveredId === prompt.id) hoveredId = null;
            }}
            onfocus={() => (hoveredId = prompt.id)}
            onblur={() => {
              if (hoveredId === prompt.id) hoveredId = null;
            }}
          >
            <span class="chat-timeline-core"></span>
          </button>

          {#if hoveredId === prompt.id}
            <div class="chat-timeline-tip" role="tooltip">
              <span class="tip-text">{getSnippet(prompt.text) || "Jump to prompt"}</span>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </nav>
{/if}

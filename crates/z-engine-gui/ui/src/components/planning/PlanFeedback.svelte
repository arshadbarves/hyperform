<script lang="ts">
  import { Button } from "$lib/ui";

  /** "What should change?": ⌘Enter sends it to the planning agent, Esc puts it away. */
  type Props = { busy: boolean; onSend: (text: string) => void; onCancel: () => void };
  let { busy, onSend, onCancel }: Props = $props();

  let text = $state("");
  const ready = $derived(text.trim().length > 0 && !busy);

  function send() {
    if (ready) onSend(text.trim());
  }
</script>

<!-- svelte-ignore a11y_autofocus -->
<textarea
  class="plan-view-feedback"
  bind:value={text}
  rows={3}
  autofocus
  aria-label="What should change in the plan"
  placeholder="What should change in the plan?"
  onkeydown={(e) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      send();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
  }}
></textarea>
<div class="plan-view-buttons">
  <Button variant="primary" disabled={!ready} onclick={send}>Send feedback</Button>
  <Button variant="ghost" disabled={busy} onclick={onCancel}>Cancel</Button>
</div>

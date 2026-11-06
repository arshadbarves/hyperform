<script lang="ts">
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { pushToast } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy, GitCompare } from "$lib/ui/icons";
  import RewindMenu from "./RewindMenu.svelte";

  /** Once, after a finished turn: copy its answer, open what it changed, rewind to before its prompt. */
  type Props = {
    answer: string;
    changed: boolean;
    prompt: Message | null;
    canRestoreCode?: boolean;
    onRewind?: (message: Message, scope: RewindScope) => void;
  };
  let { answer, changed, prompt, canRestoreCode = false, onRewind }: Props = $props();

  const feedback = copyFeedback();

  async function copy() {
    if (!(await feedback.copy(answer))) pushToast("Copy failed", "warn");
  }
</script>

{#if answer || changed || (prompt && onRewind)}
  <div class="turn-actions" role="group" aria-label="This turn">
    {#if answer}
      <button
        type="button"
        class="turn-action"
        class:is-copied={feedback.copied}
        title={feedback.copied ? "Copied" : "Copy the answer"}
        aria-label="Copy the answer"
        onclick={() => void copy()}
      >
        <Icon icon={feedback.copied ? Check : Copy} size={13} />
      </button>
    {/if}
    {#if changed}
      <button type="button" class="turn-action" title="Open changes" aria-label="Open changes" onclick={() => ui.openPanel("changes")}>
        <Icon icon={GitCompare} size={13} />
      </button>
    {/if}
    {#if prompt && onRewind}
      <RewindMenu {canRestoreCode} onRewind={(scope) => onRewind(prompt, scope)} />
    {/if}
  </div>
{/if}

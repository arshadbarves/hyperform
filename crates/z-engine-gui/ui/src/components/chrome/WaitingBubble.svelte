<script lang="ts">
  import type { WaitingChat } from "$lib/domain/liveStatus";
  import { openChatById, showInbox } from "$lib/stores/app-actions";
  import { Tooltip } from "$lib/ui";

  /** Other chats blocked on the user: one opens directly, several open the inbox. */
  type Props = { waiting: WaitingChat; more: number };
  let { waiting, more }: Props = $props();

  const count = $derived(1 + more);
  const text = $derived(count === 1 ? `${waiting.title} needs you` : `${count} chats need you`);

  function open() {
    if (count === 1) void openChatById(waiting.sessionId);
    else showInbox();
  }
</script>

<Tooltip {text}>
  <button type="button" class="satellite waiting-bubble" aria-label={text} onclick={open}>
    <span class="waiting-count">{count}</span>
  </button>
</Tooltip>

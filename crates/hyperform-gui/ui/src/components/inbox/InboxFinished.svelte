<script lang="ts">
  import type { FinishedItem } from "$lib/domain/inbox";
  import { openChatById } from "$lib/stores/app-actions";
  import { Button } from "$lib/ui";
  import { relTime } from "$lib/util";

  /** Background chats whose turn ended since you last looked at them. */
  type Props = { items: FinishedItem[] };
  let { items }: Props = $props();
</script>

<section class="inbox-section" aria-label="Finished while you were away">
  <h2 class="inbox-heading">Finished while you were away</h2>
  <ul class="inbox-list">
    {#each items as item (item.sessionId)}
      <li class="inbox-item">
        <span class={`inbox-dot tone-${item.tone}`} aria-hidden="true"></span>
        <div class="inbox-body">
          <p class="inbox-item-title">{item.chatTitle}</p>
          <p class="inbox-item-meta"><span>{item.label}</span><time>{relTime(item.at)}</time></p>
        </div>
        <div class="inbox-actions">
          <Button variant="secondary" onclick={() => void openChatById(item.sessionId)}>Open</Button>
        </div>
      </li>
    {/each}
  </ul>
</section>

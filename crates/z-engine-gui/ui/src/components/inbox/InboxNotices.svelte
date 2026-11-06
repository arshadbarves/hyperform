<script lang="ts">
  import type { InboxNotice } from "$lib/domain/inbox";
  import { openChatById } from "$lib/stores/app-actions";
  import { SegmentedChoice, copyFeedback } from "$lib/ui";
  import Icon, { Check, Copy } from "$lib/ui/icons";
  import { relTime } from "$lib/util";

  /** Every notice in full, newest first; long ones fold after a few lines. */
  type Props = { notices: InboxNotice[]; readAt: number };
  let { notices, readAt }: Props = $props();

  const FILTERS = [
    { value: "all" as const, label: "All", description: "Every notice" },
    { value: "problems" as const, label: "Problems", description: "Warnings and errors only" },
  ];
  const LONG = 220;
  let filter = $state<"all" | "problems">("all");
  let expanded = $state<Record<string, boolean>>({});
  const copier = copyFeedback(1400);
  let copiedKey = $state<string | null>(null);

  const shown = $derived(
    (filter === "all" ? notices : notices.filter((n) => n.tone === "warn" || n.tone === "error")).slice(0, 100),
  );

  function copy(notice: InboxNotice) {
    copiedKey = notice.key;
    void copier.copy(notice.text);
  }
</script>

<section class="inbox-section" aria-label="Notices">
  <div class="inbox-heading-row">
    <h2 class="inbox-heading">Notices</h2>
    <SegmentedChoice label="Show" options={FILTERS} value={filter} onSelect={(next) => (filter = next)} />
  </div>
  <ul class="inbox-list">
    {#each shown as notice (notice.key)}
      {@const long = notice.text.length > LONG}
      <li class={`inbox-item is-notice tone-${notice.tone}${notice.at > readAt ? " is-unread" : ""}`}>
        <span class={`inbox-dot tone-${notice.tone}`} aria-hidden="true"></span>
        <div class="inbox-body">
          <p class="inbox-notice-text" class:is-folded={long && !expanded[notice.key]}>{notice.text}</p>
          <p class="inbox-item-meta">
            {#if notice.sessionId && notice.chatTitle}
              <button type="button" class="inbox-link" onclick={() => notice.sessionId && void openChatById(notice.sessionId)}>
                {notice.chatTitle}
              </button>
            {/if}
            <time>{relTime(notice.at)}</time>
            {#if long}
              <button type="button" class="inbox-link" onclick={() => (expanded[notice.key] = !expanded[notice.key])}>
                {expanded[notice.key] ? "Show less" : "Show all"}
              </button>
            {/if}
          </p>
        </div>
        <button type="button" class="icon-btn-mini inbox-copy" aria-label="Copy notice" title="Copy" onclick={() => copy(notice)}>
          <Icon icon={copier.copied && copiedKey === notice.key ? Check : Copy} size={13} />
        </button>
      </li>
    {:else}
      <li class="inbox-none">No problems. Everything that went wrong would be listed here.</li>
    {/each}
  </ul>
</section>

<script lang="ts">
  import type { ChatRow } from "$lib/domain/sidebarModel";
  import Icon, { Trash2 } from "$lib/ui/icons";

  /** One chat: its mark, title and age; delete appears on hover. */
  type Props = { row: ChatRow; onOpen: () => void; onDelete: () => void };
  let { row, onOpen, onDelete }: Props = $props();

  const hint = $derived(
    [row.item.title, row.mark?.label, row.item.legacy ? "imported from v1" : null].filter(Boolean).join(" · "),
  );

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Enter" && e.key !== " ") return;
    e.preventDefault();
    onOpen();
  }
</script>

<div
  class={`chat-row${row.active ? " is-active" : ""}`}
  role="button"
  tabindex={0}
  title={hint}
  aria-current={row.active ? "page" : undefined}
  onclick={onOpen}
  onkeydown={onKey}
>
  <span class="chat-row-lead">
    {#if row.mark}<span class={`chat-mark tone-${row.mark.tone}`} role="img" aria-label={row.mark.label}></span>{/if}
  </span>
  <span class="chat-row-title">{row.item.title}</span>
  <span class="chat-row-when">{row.when}</span>
  <button
    type="button"
    class="chat-row-delete"
    aria-label={`Delete ${row.item.title}`}
    title="Delete chat"
    onclick={(e) => {
      e.stopPropagation();
      onDelete();
    }}
  >
    <Icon icon={Trash2} size={12} strokeWidth={1.8} />
  </button>
</div>

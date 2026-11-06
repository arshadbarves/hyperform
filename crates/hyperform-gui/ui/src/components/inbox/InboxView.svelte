<script lang="ts">
  import { inbox, inboxSnapshot } from "$lib/runtime";
  import { petUi } from "$lib/stores/pet.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { EmptyState } from "$lib/ui";
  import { Inbox } from "$lib/ui/icons";
  import { perch } from "$lib/ui/perch.svelte";
  import InboxFinished from "./InboxFinished.svelte";
  import InboxNeedsYou from "./InboxNeedsYou.svelte";
  import InboxNotices from "./InboxNotices.svelte";

  /**
   * The Activity inbox: what is waiting on you across every chat, what
   * finished while you looked elsewhere, and every notice in full. When
   * it is empty, a roaming pet sits (and naps) where the icon would be.
   */
  const snap = $derived(inboxSnapshot());
  const readAt = inbox.readAt;
  const petHere = $derived((settingsStore.settings?.ui.companion ?? "lively") === "lively" && petUi.roam);
  const empty = $derived(snap.needsYou.length === 0 && snap.finished.length === 0 && snap.notices.length === 0);
  const summary = $derived.by(() => {
    const parts = [
      snap.needsYou.length && `${snap.needsYou.length} waiting on you`,
      snap.finished.length && `${snap.finished.length} finished`,
    ].filter(Boolean);
    return parts.length ? parts.join(" · ") : "Nothing is waiting on you.";
  });

  $effect(() => () => inbox.markRead());
</script>

{#snippet petSpot()}
  <div class="empty-state-pet" use:perch={{ id: "empty", kind: "slot" }} aria-hidden="true"></div>
{/snippet}

<div class="inbox">
  <header class="inbox-head">
    <h1 class="inbox-title">Inbox</h1>
    <p class="inbox-sub">{summary}</p>
  </header>

  {#if empty}
    <EmptyState
      icon={Inbox}
      art={petHere ? petSpot : undefined}
      title="You're all caught up"
      description="Questions from the agent, chats that finish in the background, and every notice land here."
    />
  {:else}
    {#if snap.needsYou.length}<InboxNeedsYou items={snap.needsYou} />{/if}
    {#if snap.finished.length}<InboxFinished items={snap.finished} />{/if}
    {#if snap.notices.length}<InboxNotices notices={snap.notices} {readAt} />{/if}
  {/if}
</div>

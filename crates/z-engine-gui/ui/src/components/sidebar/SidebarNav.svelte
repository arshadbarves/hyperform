<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { goHome, showInbox, startNewChat } from "$lib/stores/app-actions";
  import { currentStage } from "$lib/stores/stage.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Badge, Kbd, SelectionCapsule } from "$lib/ui";
  import Icon, { Home, Inbox, Plus, Search } from "$lib/ui/icons";

  /** The sidebar's fixed top: start something, find something, and the two places. */
  type Props = { inboxCount: number };
  let { inboxCount }: Props = $props();

  const mod = modLabel();
  const stage = $derived(currentStage());
</script>

<div class="sidebar-top">
  <button type="button" class="sidebar-new" onclick={() => void startNewChat()}>
    <Icon icon={Plus} size={14} strokeWidth={2} />
    <span>New chat</span>
    <Kbd keys={`${mod}N`} />
  </button>
  <nav class="sidebar-nav" aria-label="Places">
    <SelectionCapsule selector=".sidebar-row.is-current" />
    <button type="button" class="sidebar-row" onclick={() => ui.openPalette()}>
      <Icon icon={Search} size={14} strokeWidth={1.8} />
      <span class="sidebar-row-label">Search</span>
      <Kbd keys={`${mod}K`} />
    </button>
    <button
      type="button"
      class={`sidebar-row${stage === "home" ? " is-current" : ""}`}
      aria-current={stage === "home" ? "page" : undefined}
      onclick={() => goHome()}
    >
      <Icon icon={Home} size={14} strokeWidth={1.8} />
      <span class="sidebar-row-label">Home</span>
    </button>
    <button
      type="button"
      class={`sidebar-row${stage === "inbox" ? " is-current" : ""}`}
      aria-current={stage === "inbox" ? "page" : undefined}
      onclick={showInbox}
    >
      <Icon icon={Inbox} size={14} strokeWidth={1.8} />
      <span class="sidebar-row-label">Inbox</span>
      <Badge count={inboxCount} tone="attention" label={`${inboxCount} waiting`} />
    </button>
  </nav>
</div>

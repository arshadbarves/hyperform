<script lang="ts">
  import { untrack } from "svelte";
  import { hasUnseen, recentActivity } from "$lib/domain/activity";
  import { contextMeter } from "$lib/domain/contextMeter";
  import { islandLine } from "$lib/domain/island";
  import { reactionPose } from "$lib/domain/pet/behavior";
  import { sessions } from "$lib/runtime";
  import { island } from "$lib/stores/island.svelte";
  import type { Live } from "$lib/stores/live.svelte";
  import { petUi } from "$lib/stores/pet.svelte";
  import { splitOff } from "$lib/ui/motion";
  import ContextBubble from "./ContextBubble.svelte";
  import Island from "./Island.svelte";
  import WaitingBubble from "./WaitingBubble.svelte";

  /**
   * The title bar's middle: the island, always centred, with two droplets
   * beside it: other chats that need you on the left, how full this chat's
   * context is on the right. They ride the capsule's edges and fold back
   * into it while it is open as a card.
   */
  type Props = { live: Live; chat: string | null };
  let { live, chat }: Props = $props();

  let capsuleWidth = $state(0);

  const view = $derived(live.view);
  const status = $derived(live.status);
  const pose = $derived(live.pose && reactionPose(live.pose, "sit", petUi.reactionAt(live.now)));

  $effect(() => {
    void sessions.activeId;
    untrack(() => (island.open = false));
  });

  const line = $derived(islandLine(status, chat));
  const meter = $derived(
    view?.info
      ? contextMeter({ contextTokens: view.contextTokens, contextLimit: view.contextLimit, breakdown: view.contextBreakdown })
      : null,
  );
  const activity = $derived(recentActivity(view));
  const unseen = $derived(!island.open && hasUnseen(activity, island.seenAt(sessions.activeId)));
  const progress = $derived(status.progress ? status.progress.done / status.progress.total : null);
  const label = $derived([line.text, status.detail, line.metric].filter(Boolean).join(", ") || "HyperForm AI");
</script>

<div
  class={`title-status tone-${status.tone}`}
  class:is-expanded={island.open}
  style:--island-cap-w={`${capsuleWidth}px`}
>
  <span class="satellite-slot is-lead">
    {#if status.waiting}
      <span class="satellite-drop" transition:splitOff={{ from: 24 }}>
        <WaitingBubble waiting={status.waiting} more={status.moreWaiting} />
      </span>
    {/if}
  </span>
  <Island
    {status}
    {line}
    {pose}
    {progress}
    notice={live.notice}
    {view}
    {activity}
    {meter}
    {unseen}
    {label}
    title={chat}
    bind:capsuleWidth
  />
  <span class="satellite-slot is-trail">
    {#if view && meter}
      <span class="satellite-drop" transition:splitOff={{ from: -24 }}>
        <ContextBubble sessionId={view.sessionId} {meter} busy={view.status !== "idle"} />
      </span>
    {/if}
  </span>
  <span class="sr-only" aria-live="polite">
    {status.kind === "idle" ? "" : [status.text, status.detail].filter(Boolean).join(", ")}
  </span>
</div>

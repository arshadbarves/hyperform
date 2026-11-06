<script lang="ts">
  import type { SessionListItem } from "$lib/domain/sessionList";
  import { sidebarMark } from "$lib/domain/sessionOutcome";
  import type { ContinuePlanCard } from "$lib/domain/work/homeCard";
  import { sessions } from "$lib/runtime";
  import { openChat } from "$lib/stores/app-actions";
  import { fmtCost, relTime } from "$lib/util";
  import ContinuePlan from "./ContinuePlan.svelte";

  /**
   * The project's latest chats, with more than the sidebar shows: where
   * each one stands and what it cost. With work plans on, the plan in
   * progress comes first.
   */
  type Props = { chats: SessionListItem[]; plan?: ContinuePlanCard | null; now: number };
  let { chats, plan = null, now }: Props = $props();

  const running = $derived(chats.filter((c) => sessions.activity[c.sessionId] === "working").length);

  function state(chat: SessionListItem): string | null {
    const mark = sidebarMark({
      active: false,
      activity: sessions.activity[chat.sessionId] ?? null,
      unread: sessions.unread[chat.sessionId],
      lastOutcome: chat.lastOutcome,
    });
    return mark?.label ?? null;
  }
</script>

<section class="home-card" aria-label="Continue">
  <header class="home-card-head">
    <h2 class="home-card-title">Continue</h2>
    {#if running}<span class="home-running">{running} working now</span>{/if}
  </header>
  {#if plan}<ContinuePlan card={plan} {now} />{/if}
  {#if chats.length}
    <ul class="continue-list">
      {#each chats as chat (chat.sessionId)}
        <li>
          <button type="button" class="continue-row" onclick={() => void openChat(chat.sessionId, chat.projectRoot)}>
            <span class="continue-title">{chat.title}</span>
            <span class="continue-meta">
              {#if state(chat)}<span class="continue-state">{state(chat)}</span>{/if}
              <span>{relTime(chat.updatedAt, now)}</span>
              {#if chat.costUsd > 0}<span>{fmtCost(chat.costUsd)}</span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

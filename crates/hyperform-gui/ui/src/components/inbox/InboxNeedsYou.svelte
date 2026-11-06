<script lang="ts">
  import type { NeedsYouItem } from "$lib/domain/inbox";
  import { resolveApproval } from "$lib/runtime";
  import { openChatById } from "$lib/stores/app-actions";
  import { openWorkPlan } from "$lib/stores/plan-actions";
  import { Button, Pill } from "$lib/ui";
  import Icon, { HelpCircle, ListChecks, Lock, Shield, Workflow } from "$lib/ui/icons";

  /**
   * Chats blocked on you, and work plans that wait on you (one item per
   * plan). Approvals can be answered right here; the rest open their
   * chat, a work plan on its Plan tab.
   */
  type Props = { items: NeedsYouItem[] };
  let { items }: Props = $props();

  const ICONS = { approval: Shield, question: HelpCircle, plan: ListChecks, trust: Lock, workPlan: Workflow } as const;
  const OPEN = { approval: "Open chat", question: "Answer", plan: "Review plan", trust: "Decide", workPlan: "Open plan" } as const;
  let busy = $state<Record<string, boolean>>({});

  function open(item: NeedsYouItem) {
    if (item.plan) void openWorkPlan(item.plan, false);
    else void openChatById(item.sessionId);
  }

  async function decide(item: NeedsYouItem, allow: boolean) {
    if (!item.requestId) return;
    busy[item.key] = true;
    const decision = allow ? ({ type: "allowOnce" } as const) : ({ type: "deny", feedback: null } as const);
    await resolveApproval(item.requestId, decision, item.sessionId);
    busy[item.key] = false;
  }
</script>

<section class="inbox-section" aria-label="Needs you">
  <h2 class="inbox-heading">Needs you <Pill tone="attention">{items.length}</Pill></h2>
  <ul class="inbox-list">
    {#each items as item (item.key)}
      <li class="inbox-item is-attention">
        <span class="inbox-icon" aria-hidden="true"><Icon icon={ICONS[item.kind]} size={15} /></span>
        <div class="inbox-body">
          <p class="inbox-item-title">{item.title}</p>
          <p class="inbox-item-meta">
            <span>{item.chatTitle}</span>
            {#if item.detail}<span class="inbox-item-detail">{item.detail}</span>{/if}
          </p>
        </div>
        <div class="inbox-actions">
          <Button onclick={() => open(item)}>{OPEN[item.kind]}</Button>
          {#if item.kind === "approval"}
            <Button variant="secondary" disabled={busy[item.key]} onclick={() => void decide(item, false)}>Deny</Button>
            <Button variant="accent" disabled={busy[item.key]} onclick={() => void decide(item, true)}>Allow once</Button>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
</section>

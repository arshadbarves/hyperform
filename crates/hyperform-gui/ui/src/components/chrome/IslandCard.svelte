<script lang="ts">
  import type { ActivityItem } from "$lib/domain/activity";
  import { workCounts } from "$lib/domain/agentTree";
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import { islandPlanRow, recentSteps } from "$lib/domain/island";
  import type { LiveStatus } from "$lib/domain/liveStatus";
  import type { SessionView } from "$lib/domain/sessionView";
  import type { PanelTab } from "$lib/domain/sidePanel";
  import { dismissToast, type Toast } from "$lib/runtime/toasts";
  import { openChatById, showInbox } from "$lib/stores/app-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button, Icon } from "$lib/ui";
  import { ChevronRight } from "$lib/ui/icons";
  import { fmtCost, fmtTokens, relTime } from "$lib/util";

  /**
   * The island grown into a card: the whole message, the plan and the live
   * steps, then a row each for helpers, context and other chats waiting.
   * Rows open what they summarize; any of them closes the card.
   */
  type Props = {
    id: string;
    closing: boolean;
    view: SessionView | null;
    status: LiveStatus;
    notice: Toast | null;
    activity: ActivityItem[];
    meter: CtxMeter | null;
    onHeight: (height: number) => void;
    onClose: () => void;
  };
  let { id, closing, view, status, notice, activity, meter, onHeight, onClose }: Props = $props();

  type Link = { id: string; label: string; value: string; tone: string; run: () => void };

  let body: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!body || closing) return;
    const el = body;
    const report = () => onHeight(el.offsetHeight);
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    return () => observer.disconnect();
  });

  const shownNotice = $derived(status.kind === "notice" ? notice : null);
  const headline = $derived(shownNotice ? (shownNotice.title ?? shownNotice.text) : (status.text ?? "Ready when you are"));
  const detail = $derived(
    shownNotice ? (shownNotice.title && shownNotice.title !== shownNotice.text ? shownNotice.text : null) : status.detail,
  );
  const plan = $derived(islandPlanRow(view));
  const steps = $derived(view?.activeTurn ? recentSteps(view.tools, view.activeTurn.startedAt, 4) : []);
  const turnCost = $derived(view?.activeTurn ? view.costUsd - view.activeTurn.costAtStart : null);
  const warnings = $derived(activity.filter((a) => a.tone !== "info").slice(0, 3));

  const links = $derived.by(() => {
    const out: Link[] = [];
    const c = view ? workCounts(view.agents, view.jobs, view.work) : null;
    const helpers = c
      ? [
          c.runningAgents && `${c.runningAgents} working`,
          c.runningJobs && `${c.runningJobs} job${c.runningJobs === 1 ? "" : "s"} running`,
          c.pendingWorktrees && `${c.pendingWorktrees} ready to apply`,
        ].filter(Boolean)
      : [];
    if (helpers.length) out.push(link("agents", "Agents", helpers.join(" · "), "quiet"));
    if (meter) {
      const value = `${meter.pct}% · ${fmtTokens(meter.used)} of ${fmtTokens(meter.max)}`;
      out.push(link("context", "Context", value, meter.level));
    }
    const waiting = status.waiting;
    if (waiting) {
      const many = status.moreWaiting > 0;
      out.push({
        id: "waiting",
        label: "Waiting",
        value: many ? `${status.moreWaiting + 1} other chats need you` : `${waiting.title} needs you`,
        tone: "attention",
        run: () => (many ? showInbox() : void openChatById(waiting.sessionId)),
      });
    }
    return out;
  });

  function link(tab: PanelTab, label: string, value: string, tone: string): Link {
    return { id: tab, label, value, tone, run: () => ui.openPanel(tab) };
  }

  function go(run: () => void) {
    onClose();
    run();
  }
</script>

<div {id} class="island-card" class:is-closing={closing} role="region" aria-label="Status details">
  <div class="island-card-body" bind:this={body}>
    <header class="island-card-head">
      <p class={`island-card-step tone-${status.tone}`}>{headline}</p>
      {#if detail}<p class="island-card-detail">{detail}</p>{/if}
      {#if shownNotice}
        <div class="island-card-actions">
          {#each shownNotice.actions ?? [] as action (action.label)}
            <Button
              variant={action.variant === "primary" ? "accent" : "secondary"}
              size="s"
              onclick={() =>
                go(() => {
                  action.onclick?.();
                  dismissToast(shownNotice.id);
                })}
            >
              {action.label}
            </Button>
          {/each}
          <Button size="s" onclick={() => go(showInbox)}>Open in Inbox</Button>
        </div>
      {/if}
    </header>

    {#if plan}
      <section class="island-card-section">
        <button type="button" class="island-card-link" onclick={() => go(() => ui.openPanel("plan"))}>
          <span class="island-card-label">Plan</span>
          <span class="island-card-value">{plan.label}</span>
          <Icon icon={ChevronRight} size={13} class="island-card-chevron" />
        </button>
        <span class="island-card-bar" aria-hidden="true">
          <span style:scale={`${plan.done / plan.total} 1`}></span>
        </span>
      </section>
    {/if}

    {#if steps.length}
      <section class="island-card-section">
        <h3 class="island-card-label">Now</h3>
        <ul class="island-card-steps">
          {#each steps as step (step.callId)}
            <li class={`is-${step.state}`}>{step.label}</li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if links.length}
      <section class="island-card-section island-card-links">
        {#each links as item (item.id)}
          <button type="button" class="island-card-link" onclick={() => go(item.run)}>
            <span class="island-card-label">{item.label}</span>
            <span class={`island-card-value tone-${item.tone}`}>{item.value}</span>
            <Icon icon={ChevronRight} size={13} class="island-card-chevron" />
          </button>
        {/each}
      </section>
    {/if}

    {#if view && view.costUsd > 0}
      <section class="island-card-section island-card-row">
        <span class="island-card-label">Cost</span>
        <span class="island-card-value">
          {#if turnCost !== null}This turn {fmtCost(turnCost)} · {/if}Chat {fmtCost(view.costUsd)}
        </span>
      </section>
    {/if}

    {#if warnings.length}
      <section class="island-card-section">
        <div class="island-card-row">
          <h3 class="island-card-label">Recent</h3>
          <button type="button" class="island-card-more" onclick={() => go(showInbox)}>Open in Inbox</button>
        </div>
        <ul class="island-card-activity">
          {#each warnings as item (item.key)}
            <li class={`tone-${item.tone}`}>
              <span class="island-card-activity-text">{item.text}</span>
              <time class="island-card-activity-time">{relTime(item.at)}</time>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </div>
</div>

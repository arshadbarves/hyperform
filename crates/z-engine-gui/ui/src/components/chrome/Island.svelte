<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { ActivityItem } from "$lib/domain/activity";
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import {
    islandAction,
    islandCard,
    islandMode,
    islandShape,
    noticeOpensIsland,
    type IslandLine,
  } from "$lib/domain/island";
  import type { LiveStatus } from "$lib/domain/liveStatus";
  import type { PetPose } from "$lib/domain/pet/pose";
  import type { SessionView } from "$lib/domain/sessionView";
  import type { Toast } from "$lib/runtime/toasts";
  import { island } from "$lib/stores/island.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { presence } from "$lib/ui";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import IslandCapsule from "./IslandCapsule.svelte";
  import IslandCard from "./IslandCard.svelte";

  /**
   * The title bar's island: one glass surface that changes shape. It rests as
   * a capsule with the pet, says what is happening while live, turns amber
   * with one button when this chat needs you, and grows into a card when
   * opened. A hidden copy of the row sets the capsule's width (clamped to the
   * title bar's room, reported as `capsuleWidth` so the satellites follow);
   * the stylesheet springs the surface between shapes.
   */
  type Props = {
    status: LiveStatus;
    line: IslandLine;
    pose: PetPose | null;
    progress: number | null;
    notice: Toast | null;
    view: SessionView | null;
    activity: ActivityItem[];
    meter: CtxMeter | null;
    unseen: boolean;
    label: string;
    title: string | null;
    capsuleWidth?: number;
  };
  let {
    status,
    line,
    pose,
    progress,
    notice,
    view,
    activity,
    meter,
    unseen,
    label,
    title,
    capsuleWidth = $bindable(0),
  }: Props = $props();

  const CARD_ID = "island-card";
  let slot: HTMLDivElement | undefined = $state();
  let spacer: HTMLSpanElement | undefined = $state();
  let surface: HTMLDivElement | undefined = $state();
  let toggle: HTMLButtonElement | undefined = $state();
  let cardHeight = $state(0);
  let viewportW = $state(1280);
  let viewportH = $state(800);
  let ready = $state(false);
  let autoOpened = $state(false);
  let hovering = $state(false);
  let lastNotice: number | null = null;

  const mode = $derived(islandMode(status, island.open));
  const action = $derived(islandAction(status));
  const room = $derived({ capsule: capsuleWidth, card: cardHeight, viewportW, viewportH });
  const shape = $derived(islandShape(mode, room));
  const card = $derived(islandCard(room));
  const cardPresence = presence(() => island.open);

  $effect(() => {
    if (!slot) return;
    const el = slot;
    const observer = new ResizeObserver(() => {
      capsuleWidth = Math.ceil(el.getBoundingClientRect().width);
      if (!untrack(() => ready)) requestAnimationFrame(() => (ready = true));
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  $effect(() => {
    const current = notice;
    if (!current || current.id === lastNotice) return;
    lastNotice = current.id;
    requestAnimationFrame(() => {
      const truncated = !!spacer && spacer.scrollWidth > spacer.clientWidth + 1;
      if (island.open || !noticeOpensIsland(current, truncated)) return;
      autoOpened = true;
      island.open = true;
    });
  });

  $effect(() => {
    if (island.open && autoOpened && notice === null && !hovering) close();
  });

  $effect(() => {
    if (!island.open) return;
    const sessionId = view?.sessionId ?? null;
    untrack(() => {
      island.markSeen(sessionId);
      userSignals.dismissRecap();
    });
  });

  // Opened, a press anywhere else or Esc closes it. Esc is caught first and
  // spent here, so the same press doesn't also close the side panel.
  $effect(() => {
    if (!island.open) return;
    const onPointer = (e: PointerEvent) => {
      if (!surface?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      if (document.querySelector("[role='dialog'], [role='alertdialog']")) return;
      e.preventDefault();
      e.stopPropagation();
      close();
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("keydown", onKey, true);
    };
  });

  // However it closes, focus inside the card goes back to the capsule before the card leaves.
  $effect(() => {
    if (island.open) return;
    untrack(() => {
      const active = document.activeElement;
      if (active && active !== toggle && surface?.contains(active)) toggle?.focus({ preventScroll: true });
    });
  });

  function close() {
    island.open = false;
    autoOpened = false;
  }

  function onToggle() {
    if (island.open && autoOpened) {
      autoOpened = false;
      return;
    }
    if (island.open) close();
    else island.open = true;
  }

  function onSurfaceKey(e: KeyboardEvent) {
    if (e.key !== "Escape" || !island.open) return;
    e.preventDefault();
    e.stopPropagation();
    close();
  }

  /** The card that waits for you: scrolled to, and focused where that cannot answer it by accident. */
  async function revealPending() {
    if (island.open) close();
    if (ui.view !== "chat") {
      ui.view = "chat";
      await tick();
    }
    const card = document.querySelector<HTMLElement>("[data-pending-card]");
    if (!card) return;
    card.scrollIntoView({ block: "center", behavior: prefersReducedMotion() ? "auto" : "smooth" });
    const target =
      card.tabIndex >= 0 ? card : card.querySelector<HTMLElement>("textarea, input, select, [role='radio'], [role='option']");
    target?.focus({ preventScroll: true });
  }
</script>

<svelte:window bind:innerWidth={viewportW} bind:innerHeight={viewportH} />

<div
  bind:this={slot}
  class="island"
  class:is-ready={ready}
  class:has-text={!!line.text}
  class:has-action={!!action}
  data-mode={mode}
  data-tone={status.tone}
>
  <span class="island-row island-spacer" aria-hidden="true" bind:this={spacer}>
    <span class="island-pet-space"></span>
    {#if line.text}<span class="island-text">{line.text}</span>{/if}
    {#if line.metric}<span class="island-metric">{line.metric}</span>{/if}
    {#if unseen}<span class="island-unseen"></span>{/if}
    {#if action}<span class="island-action">{action.label}</span>{/if}
  </span>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={surface}
    class="island-surface"
    style:width={`${shape.width}px`}
    style:height={`${shape.height}px`}
    style:border-radius={`${shape.radius}px`}
    style:--island-card-w={`${card.width}px`}
    style:--island-card-max={`${card.bodyMax}px`}
    onpointerenter={() => (hovering = true)}
    onpointerleave={() => (hovering = false)}
    onkeydown={onSurfaceKey}
  >
    <IslandCapsule
      tone={status.tone}
      text={island.open ? title : line.text}
      metric={line.metric}
      {pose}
      {progress}
      helpers={status.helpers.running}
      sweeping={status.kind === "working" && status.activity === "compact"}
      {unseen}
      {action}
      expanded={island.open}
      controls={cardPresence.mounted ? CARD_ID : undefined}
      {label}
      bind:toggle
      {onToggle}
      onAction={() => void revealPending()}
    />
    {#if cardPresence.mounted}
      <IslandCard
        id={CARD_ID}
        closing={cardPresence.closing}
        {view}
        {status}
        {notice}
        {activity}
        {meter}
        onHeight={(height) => (cardHeight = height)}
        onClose={close}
      />
    {/if}
  </div>
</div>

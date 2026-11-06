<script lang="ts">
  import { untrack } from "svelte";
  import { clampPanelWidth, PANEL_MIN_W } from "$lib/domain/sidePanel";
  import { chatChanges, sessions } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { prefersReducedMotion, springEasingCss } from "$lib/ui/motion";
  import { springs } from "$lib/ui/springs";
  import WorkPanel from "../agents/WorkPanel.svelte";
  import DiffPanel from "../overlays/DiffPanel.svelte";
  import PromptInspector from "../overlays/PromptInspector.svelte";
  import PlanView from "../planning/PlanView.svelte";
  import SidePanelTabs from "./SidePanelTabs.svelte";

  /**
   * The right-hand panel: a glass card beside the stage, mirroring the
   * sidebar, with the Changes, Plan, Agents and Context tabs. Drag its left
   * edge to resize, or expand it over the stage. It slides in and out on
   * transform and opacity only; the stage makes room once when it mounts and
   * once when it has gone. Esc steps back: out of an agent or a plan task,
   * then out of the expanded view, then closed.
   */
  type Props = { isClosing?: boolean; room: number };
  let { isClosing = false, room }: Props = $props();

  let card: HTMLElement | undefined = $state();
  let resizing = $state(false);
  const tab = $derived(ui.panel.tab);
  const expanded = $derived(ui.panel.expanded);
  const width = $derived(clampPanelWidth(ui.panel.width, room > 0 ? room : undefined));
  const maxWidth = $derived(clampPanelWidth(Number.POSITIVE_INFINITY, room > 0 ? room : undefined));
  const changed = $derived(chatChanges.count(sessions.activeId));

  // Docking or expanding changes the layout once; the card then settles from a touch smaller.
  let shape = untrack(() => expanded);
  $effect(() => {
    const now = expanded;
    if (now === shape) return;
    shape = now;
    if (!card || isClosing || prefersReducedMotion()) return;
    card.animate([{ opacity: 0.5, transform: "scale(0.985)" }, { opacity: 1, transform: "none" }], {
      duration: springs.smooth.duration,
      easing: springEasingCss("smooth"),
    });
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape" || e.defaultPrevented || isClosing) return;
      if (ui.settingsOpen || ui.paletteOpen || ui.worktreeOpen) return;
      if ((e.target as HTMLElement | null)?.closest("textarea, input, [role='dialog'], [role='menu'], [role='listbox']")) return;
      e.preventDefault();
      const planId = sessions.active?.work?.plan.planId;
      if (ui.panel.tab === "agents" && ui.agentTranscript) ui.agentTranscript = null;
      else if (ui.panel.tab === "plan" && ui.plan.pickingFor) ui.plan.pick(null);
      else if (ui.panel.tab === "plan" && planId && ui.plan.back(planId)) return;
      else if (ui.panel.expanded) ui.setPanelExpanded(false);
      else ui.closePanel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    resizing = true;
    const startX = e.clientX;
    const startWidth = width;
    const move = (ev: PointerEvent) => ui.setPanelWidth(startWidth + (startX - ev.clientX), room);
    const end = () => {
      resizing = false;
      ui.setPanelWidth(ui.panel.width, room, true);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  function resizeByKey(e: KeyboardEvent) {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    ui.setPanelWidth(width + (e.key === "ArrowLeft" ? 24 : -24), room, true);
  }
</script>

<aside
  bind:this={card}
  class="side-panel glass"
  class:is-closing={isClosing}
  class:is-expanded={expanded}
  class:is-resizing={resizing}
  style:width={expanded ? null : `${width}px`}
  aria-label="Side panel"
>
  {#if !expanded}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="side-panel-resize"
      role="separator"
      tabindex="0"
      aria-orientation="vertical"
      aria-label="Resize the panel"
      aria-valuenow={width}
      aria-valuemin={PANEL_MIN_W}
      aria-valuemax={maxWidth}
      onpointerdown={startResize}
      onkeydown={resizeByKey}
    ></div>
  {/if}

  <SidePanelTabs
    current={tab}
    badges={ui.panelBadges}
    counts={{ changes: changed }}
    {expanded}
    onSelect={(next) => ui.showPanelTab(next)}
    onToggleExpand={() => ui.setPanelExpanded(!expanded)}
    onClose={() => ui.closePanel()}
  />

  <div class="side-panel-body" id="side-panel-body" role="tabpanel">
    {#if tab === "changes"}
      <DiffPanel />
    {:else if tab === "plan"}
      <PlanView />
    {:else if tab === "agents"}
      <WorkPanel />
    {:else}
      <PromptInspector {expanded} />
    {/if}
  </div>
</aside>

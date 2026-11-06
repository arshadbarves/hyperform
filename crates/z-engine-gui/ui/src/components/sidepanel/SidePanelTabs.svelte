<script lang="ts">
  import { panelTabs, stepTab, type PanelTab } from "$lib/domain/sidePanel";
  import { SelectionCapsule, Tooltip } from "$lib/ui";
  import { perch } from "$lib/ui/perch.svelte";
  import Icon, { Bot, Brain, Expand, GitCompare, ListChecks, Shrink, X, type IconSvgElement } from "$lib/ui/icons";

  /**
   * The panel's top band: one tab per view, a dot on a tab with news, and
   * expand / close at the end. ← and → move between tabs.
   */
  type Props = {
    current: PanelTab;
    badges: readonly PanelTab[];
    counts: Partial<Record<PanelTab, number>>;
    expanded: boolean;
    onSelect: (tab: PanelTab) => void;
    onToggleExpand: () => void;
    onClose: () => void;
  };
  let { current, badges, counts, expanded, onSelect, onToggleExpand, onClose }: Props = $props();

  const ICONS: Record<PanelTab, IconSvgElement> = { changes: GitCompare, plan: ListChecks, agents: Bot, context: Brain };
  const tabs = panelTabs();
  let strip: HTMLElement | undefined = $state();

  function onKey(e: KeyboardEvent) {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const next = stepTab(current, e.key === "ArrowRight" ? 1 : -1);
    onSelect(next);
    strip?.querySelector<HTMLElement>(`[data-tab="${next}"]`)?.focus();
  }
</script>

<header class="side-panel-head" data-tauri-drag-region>
  <div class="side-panel-tabs" role="tablist" aria-label="Panel" tabindex="-1" bind:this={strip} onkeydown={onKey}>
    <SelectionCapsule selector=".side-panel-tab.is-current" />
    {#each tabs as tab (tab.id)}
      {@const on = tab.id === current}
      <button
        type="button"
        role="tab"
        class="side-panel-tab"
        class:is-current={on}
        data-tab={tab.id}
        aria-selected={on}
        aria-controls="side-panel-body"
        tabindex={on ? 0 : -1}
        title={tab.label}
        onclick={() => onSelect(tab.id)}
      >
        <Icon icon={ICONS[tab.id]} size={13} strokeWidth={1.8} />
        <span class="side-panel-tab-label">{tab.label}</span>
        {#if counts[tab.id]}<span class="side-panel-tab-count">{counts[tab.id]}</span>{/if}
        {#if badges.includes(tab.id)}<span class="side-panel-tab-badge" aria-label="new"></span>{/if}
      </button>
    {/each}
  </div>
  <!-- The pet's perch on the panel: the band's free space, clear of the tabs. -->
  <span class="side-panel-head-space" data-tauri-drag-region use:perch={{ id: "panel", kind: "slot" }}></span>
  <Tooltip text={expanded ? "Dock beside the chat" : "Use the whole stage"}>
    <button type="button" class="icon-btn" aria-label={expanded ? "Dock beside the chat" : "Use the whole stage"} onclick={onToggleExpand}>
      <Icon icon={expanded ? Shrink : Expand} size={14} />
    </button>
  </Tooltip>
  <Tooltip text="Close" shortcut="Esc">
    <button type="button" class="icon-btn" aria-label="Close the panel" onclick={onClose}>
      <Icon icon={X} size={14} />
    </button>
  </Tooltip>
</header>

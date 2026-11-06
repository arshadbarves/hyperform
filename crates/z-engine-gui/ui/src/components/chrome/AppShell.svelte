<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { untrack } from "svelte";
  import { sessionLabel, viewTitle } from "$lib/domain/sessionList";
  import { workPlansOn } from "$lib/domain/work/panel";
  import { inboxSnapshot, projectPlans, projects, sessions } from "$lib/runtime";
  import { createWorktreeAndStart, openSettings, startNewChat } from "$lib/stores/app-actions";
  import { createLive } from "$lib/stores/live.svelte";
  import { followPanelNudges } from "$lib/stores/panelNudges.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { presence } from "$lib/ui/presence.svelte";
  import { workspaceStore } from "$lib/workspaces";
  import WorktreeDialog from "../overlays/WorktreeDialog.svelte";
  import PetLayer from "../pet/PetLayer.svelte";
  import SidePanel from "../sidepanel/SidePanel.svelte";
  import AppSidebar from "../sidebar/AppSidebar.svelte";
  import MainStage from "./MainStage.svelte";
  import TopBar from "./TopBar.svelte";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  /**
   * The window once set up: the floating sidebar card, the stage with the
   * transparent title zone over it, the side panel card, and the pet, which
   * roams above them. Windows' caption buttons keep the window's corner.
   */
  type Props = { entering: boolean };
  let { entering }: Props = $props();

  const workspaces = bindStore(workspaceStore);
  const panelPresence = presence(() => ui.panel.open);
  const live = createLive();
  let room = $state(0);

  const view = $derived(sessions.active);
  const chatTitle = $derived(view ? sessionLabel(viewTitle(view)) : null);
  const inboxCount = $derived(inboxSnapshot().count);
  /** The panel takes room beside the stage (docked, and still while it leaves). */
  const docked = $derived(panelPresence.mounted && !ui.panel.expanded);

  followPanelNudges();

  $effect(() => {
    void projects.refreshAll(workspaces.current.roots);
  });

  // Work plans (picker, home card, Inbox) load only while the feature is on.
  $effect(() => {
    const on = workPlansOn(settingsStore.settings?.experimental);
    const roots = workspaces.current.roots;
    untrack(() => {
      projectPlans.setEnabled(on);
      if (on) projectPlans.refreshAll(roots);
    });
  });

  $effect(() => projects.watchFocus(() => workspaceStore.getSnapshot().roots));

  $effect(() => {
    projects.noteTurns(sessions.views, workspaces.current.roots);
  });

  $effect(() => {
    // Tauri's drag script already maximizes on a double-click that lands
    // on a data-tauri-drag-region element itself; this covers what's inside.
    function onDblClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.hasAttribute("data-tauri-drag-region")) return;
      if (target.closest("button, input, textarea, a, [role='button'], [role='tab']")) return;
      if (target.closest(".app-titlebar, .sidebar-head, .side-panel-head, .full-page-bar")) {
        void getCurrentWindow().toggleMaximize();
      }
    }
    window.addEventListener("dblclick", onDblClick);
    return () => window.removeEventListener("dblclick", onDblClick);
  });
</script>

<main
  class="app"
  class:no-sidebar={!ui.sidebarOpen}
  class:app-enter={entering}
  class:has-panel={docked}
  class:panel-expanded={ui.panelExpanded}
>
  <div class="app-body">
    <AppSidebar {inboxCount} onHide={() => (ui.sidebarOpen = false)} />

    <section class="workstation-stage" bind:clientWidth={room}>
      <div class="stage-main">
        <MainStage />
        <TopBar
          {live}
          {chatTitle}
          sidebarOpen={ui.sidebarOpen}
          onToggleSidebar={() => (ui.sidebarOpen = !ui.sidebarOpen)}
          onPalette={() => ui.openPalette()}
          onNewChat={() => void startNewChat()}
          onSettings={() => openSettings()}
        />
      </div>

      {#if panelPresence.mounted}
        <SidePanel isClosing={panelPresence.closing} {room} />
      {/if}
    </section>
  </div>

  <div class="shell-caption"><WindowControlsMaybe /></div>
  <PetLayer {live} />
</main>

{#if ui.worktreeOpen}
  <WorktreeDialog
    projects={workspaces.current.roots}
    project={workspaces.current.active}
    onClose={() => (ui.worktreeOpen = false)}
    onCreate={createWorktreeAndStart}
  />
{/if}

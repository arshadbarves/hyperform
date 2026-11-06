<script lang="ts">
  import AppShell from "./components/chrome/AppShell.svelte";
  import SplashScreen from "./components/chrome/SplashScreen.svelte";
  import Onboarding from "./components/onboarding/Onboarding.svelte";
  import CommandPalette from "./components/overlays/CommandPalette.svelte";
  import SettingsPage from "./components/settings/SettingsPage.svelte";
  import { needsOnboarding } from "./lib/domain/onboarding";
  import { paletteActions } from "./lib/paletteActions";
  import { initEvents, pet, sessionList, sessions } from "./lib/runtime";
  import { addWorkspace, openChat, openSettings, startNewChat } from "./lib/stores/app-actions";
  import { confirmStore } from "./lib/stores/confirm.svelte";
  import { onboarding } from "./lib/stores/onboarding.svelte";
  import { settingsStore } from "./lib/stores/settings.svelte";
  import { shortcutFor, type ShortcutAction } from "./lib/stores/shortcuts";
  import { ui } from "./lib/stores/ui.svelte";
  import { userSignals } from "./lib/stores/userSignals.svelte";
  import { bindStore } from "./lib/svelte/bind.svelte";
  import { ConfirmDialog } from "./lib/ui";
  import { presence } from "./lib/ui/presence.svelte";
  import { updateStore } from "./lib/updateStore";
  import { workspaceStore } from "./lib/workspaces";

  const workspaces = bindStore(workspaceStore);

  let splash = $state(true);
  let booted = $state(false);

  const palettePresence = presence(() => ui.paletteOpen, 180);

  const projectRoot = $derived(sessions.active?.info?.projectRoot ?? workspaces.current.active);

  const SHORTCUTS: Record<ShortcutAction, () => void> = {
    palette: () => ui.togglePalette(),
    newChat: () => void startNewChat(),
    toggleSidebar: () => (ui.sidebarOpen = !ui.sidebarOpen),
    toggleChanges: () => ui.togglePanel("changes"),
    settings: () => (ui.settingsOpen ? (ui.settingsOpen = false) : openSettings()),
  };

  $effect(() => {
    void (async () => {
      try {
        void pet.load();
        await initEvents();
        await workspaceStore.load();
        await sessionList.refresh();
        await settingsStore.init(projectRoot ?? null);
        const fresh = needsOnboarding({
          projects: workspaceStore.getSnapshot().roots.length,
          chats: sessionList.summaries.length,
        });
        if (fresh) onboarding.start(settingsStore.settings?.ui.pet ?? null);
      } catch (e) {
        console.error("boot failed", e);
      } finally {
        booted = true;
      }
      void updateStore.check();
    })();
  });

  function splashDone() {
    splash = false;
    userSignals.greet();
  }

  $effect(() => {
    const root = projectRoot ?? null;
    if (booted) void settingsStore.ensure(root);
  });

  $effect(() => userSignals.track());

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const action = shortcutFor(e);
      if (!action) return;
      e.preventDefault();
      SHORTCUTS[action]();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if splash}
  <SplashScreen ready={booted} onDone={splashDone} />
{/if}

{#if booted}
  {#if onboarding.active}
    <Onboarding entering={!splash} />
  {:else}
    <AppShell entering={!splash} />
  {/if}
{/if}

{#if palettePresence.mounted}
  <CommandPalette
    isClosing={palettePresence.closing}
    onClose={() => (ui.paletteOpen = false)}
    sessions={sessionList.items}
    sessionsOnly={ui.paletteSessionsOnly}
    workspaces={workspaces.current.roots}
    activeWorkspace={workspaces.current.active}
    actions={paletteActions({
      newTask: () => void startNewChat(),
      addWorkspace: () => void addWorkspace(),
      openWorktree: () => ui.openWorktree(),
      openSettings: () => openSettings(),
      toggleSidebar: () => (ui.sidebarOpen = !ui.sidebarOpen),
    })}
    onOpenSession={(id, root) => void openChat(id, root)}
    onActivateWorkspace={(root) => workspaceStore.setActive(root)}
  />
{/if}
{#if ui.settingsOpen}
  <SettingsPage onClose={() => (ui.settingsOpen = false)} />
{/if}

<ConfirmDialog
  open={confirmStore.request !== null}
  title={confirmStore.request?.title ?? ""}
  description={confirmStore.request?.description}
  confirmLabel={confirmStore.request?.confirmLabel}
  tone={confirmStore.request?.tone}
  onConfirm={() => confirmStore.settle(true)}
  onCancel={() => confirmStore.settle(false)}
/>

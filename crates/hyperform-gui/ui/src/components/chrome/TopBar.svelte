<script lang="ts">
  import { modLabel } from "$lib/platform";
  import type { Live } from "$lib/stores/live.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { PanelLeft, PanelRight, Plus, Search, Settings } from "$lib/ui/icons";
  import { updateStore } from "$lib/updateStore";
  import ChangesButton from "./ChangesButton.svelte";
  import TitleStatus from "./TitleStatus.svelte";
  import TitlebarButton from "./TitlebarButton.svelte";

  /**
   * The stage's title zone, transparent over the top of the content sheet:
   * the split island in the middle, Changes and the side panel toggle at
   * the right. While the sidebar is hidden, what lives in it (its toggle,
   * New chat, Search, Settings) joins the zone.
   */
  type Props = {
    live: Live;
    chatTitle: string | null;
    sidebarOpen: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onNewChat: () => void;
    onSettings: () => void;
  };

  let { live, chatTitle, sidebarOpen, onToggleSidebar, onPalette, onNewChat, onSettings }: Props = $props();

  const update = bindStore(updateStore);
  const mod = modLabel();
</script>

<header class="app-titlebar" data-tauri-drag-region>
  <div class="titlebar-side" data-tauri-drag-region>
    {#if !sidebarOpen}
      <TitlebarButton label="Show sidebar" shortcut={`${mod}B`} icon={PanelLeft} onclick={onToggleSidebar} />
      <TitlebarButton label="New chat" shortcut={`${mod}N`} icon={Plus} onclick={onNewChat} />
      <TitlebarButton label="Search and commands" shortcut={`${mod}K`} icon={Search} onclick={onPalette} />
    {/if}
  </div>

  <div class="titlebar-center">
    <TitleStatus {live} chat={chatTitle} />
  </div>

  <div class="titlebar-side end" data-tauri-drag-region>
    <ChangesButton />
    {#if !sidebarOpen}
      <span class="titlebar-slot">
        <TitlebarButton
          label={update.current.info?.available ? "Settings · update available" : "Settings"}
          shortcut={`${mod},`}
          icon={Settings}
          dot={Boolean(update.current.info?.available)}
          onclick={onSettings}
        />
      </span>
    {/if}
    <TitlebarButton
      label={ui.panel.open ? "Hide the side panel" : "Show the side panel"}
      icon={PanelRight}
      pressed={ui.panel.open}
      onclick={() => ui.togglePanel()}
    />
  </div>
</header>

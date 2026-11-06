<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { SettingEntry } from "$lib/domain/settings/searchIndex";
  import { layerError } from "$lib/domain/settings/provenance";
  import { activeProjectRoot } from "$lib/runtime";
  import { featureStore } from "$lib/stores/features.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import { prefersReducedMotion } from "$lib/ui/motion";
  import FullPage from "../chrome/FullPage.svelte";
  import AboutTab from "./AboutTab.svelte";
  import AdvancedTab from "./AdvancedTab.svelte";
  import AppearanceTab from "./AppearanceTab.svelte";
  import ExperimentalTab from "./ExperimentalTab.svelte";
  import ExtensionsTab from "./ExtensionsTab.svelte";
  import HooksTab from "./HooksTab.svelte";
  import McpTab from "./McpTab.svelte";
  import MemoryTab from "./MemoryTab.svelte";
  import ModelsTab from "./ModelsTab.svelte";
  import PermissionsTab from "./PermissionsTab.svelte";
  import PetTab from "./PetTab.svelte";
  import ProvidersTab from "./ProvidersTab.svelte";
  import ScopeMenu from "./ScopeMenu.svelte";
  import SettingsNav, { SETTINGS_TABS } from "./SettingsNav.svelte";
  import SettingsNotices from "./SettingsNotices.svelte";
  import VerificationTab from "./VerificationTab.svelte";

  /** Settings: pages by topic on the left, one page on the right; a search result scrolls to its row. */
  type Props = { onClose: () => void };
  let { onClose }: Props = $props();

  const update = bindStore(updateStore);
  const tab = $derived(ui.settingsTab);
  const active = $derived(SETTINGS_TABS.find((t) => t.id === tab) ?? SETTINGS_TABS[0]);
  const settings = $derived(settingsStore.settings);
  const skipped = $derived(settingsStore.provenance ? layerError(settingsStore.provenance, settingsStore.scope) : null);
  let content: HTMLElement | undefined = $state();

  $effect(() => {
    const root = activeProjectRoot();
    untrack(() => void Promise.all([settingsStore.open(root), featureStore.ensure()]));
  });

  // A search result (here or in the palette): once its page and folded
  // group have rendered, bring the row into view and mark it briefly.
  $effect(() => {
    const key = ui.settingsFocus;
    if (!key || !settings) return;
    let timer = 0;
    void tick().then(() => {
      timer = window.setTimeout(() => {
        const row = content?.querySelector<HTMLElement>(`[data-setting="${CSS.escape(key)}"]`);
        row?.scrollIntoView({ block: "center", behavior: prefersReducedMotion() ? "auto" : "smooth" });
        row?.classList.add("is-found");
        window.setTimeout(() => row?.classList.remove("is-found"), 1800);
        ui.settingsFocus = null;
      }, 60);
    });
    return () => window.clearTimeout(timer);
  });

  function jump(entry: SettingEntry) {
    ui.settingsTab = entry.tab;
    ui.settingsFocus = entry.key;
  }
</script>

<FullPage title="Settings" {onClose}>
  <SettingsNav
    {tab}
    version={settingsStore.info?.version ?? null}
    updateAvailable={Boolean(update.current.info?.available)}
    onSelect={(next) => (ui.settingsTab = next)}
    onJump={jump}
  />

  <section class="settings-pane sheet" aria-labelledby="settings-page-title">
    <header class="settings-pane-head">
      <div class="settings-pane-copy">
        <h2 id="settings-page-title">{active.label}</h2>
        <p>{active.hint}</p>
      </div>
      {#if active.scoped && settings}<ScopeMenu />{/if}
    </header>

    <div class="settings-content" bind:this={content}>
      {#key tab}
        <div class="settings-content-body">
          {#if tab !== "about"}<SettingsNotices />{/if}
          {#if active.scoped && settings && !settingsStore.root}
            <p class="form-note">Open a project to edit its project and personal settings.</p>
          {/if}
          {#if active.scoped && skipped}
            <p class="setting-error" role="alert">This settings file is not applied until it is fixed: {skipped}</p>
          {/if}
          {#if tab === "about"}
            <AboutTab />
          {:else if !settings}
            <p class="settings-loading">{settingsStore.loadError ? "Settings are unavailable." : "Loading settings…"}</p>
          {:else if tab === "models"}
            <ModelsTab {settings} />
          {:else if tab === "providers"}
            <ProvidersTab {settings} />
          {:else if tab === "permissions"}
            <PermissionsTab {settings} />
          {:else if tab === "hooks"}
            <HooksTab />
          {:else if tab === "extensions"}
            <ExtensionsTab />
          {:else if tab === "mcp"}
            <McpTab />
          {:else if tab === "verification"}
            <VerificationTab {settings} />
          {:else if tab === "memory"}
            <MemoryTab />
          {:else if tab === "advanced"}
            <AdvancedTab {settings} />
          {:else if tab === "experimental"}
            <ExperimentalTab {settings} />
          {:else if tab === "pet"}
            <PetTab {settings} />
          {:else}
            <AppearanceTab {settings} />
          {/if}
        </div>
      {/key}
    </div>
  </section>
</FullPage>

<script lang="ts" module>
  import type { SettingsTab } from "$lib/stores/ui.svelte";
  import {
    Book,
    Bot,
    Brain,
    Eye,
    Info,
    Lightbulb,
    ListChecks,
    Server,
    Shield,
    Sliders,
    Smile,
    Sparkles,
    Workflow,
    type IconSvgElement,
  } from "$lib/ui/icons";

  export interface TabMeta {
    id: SettingsTab;
    label: string;
    hint: string;
    icon: IconSvgElement;
    /** Edits layered settings, so the page shows which file it writes. */
    scoped: boolean;
  }

  export const SETTINGS_TABS: readonly TabMeta[] = [
    { id: "models", label: "Models", hint: "Which models answer, and how hard they think.", icon: Brain, scoped: true },
    { id: "providers", label: "Providers", hint: "The model services you use and their API keys.", icon: Sparkles, scoped: true },
    { id: "appearance", label: "Appearance", hint: "How much detail finished turns show, and the response style.", icon: Eye, scoped: true },
    { id: "pet", label: "Pet", hint: "Its name and look, how lively it is, and whether it roams.", icon: Smile, scoped: true },
    { id: "permissions", label: "Permissions", hint: "What the agent may do without asking.", icon: Shield, scoped: true },
    { id: "memory", label: "Memory", hint: "AGENTS.md and the other instructions every prompt includes.", icon: Book, scoped: false },
    { id: "verification", label: "Verification", hint: "The checks that show a change works.", icon: ListChecks, scoped: true },
    { id: "extensions", label: "Agents & Commands", hint: "Custom agents, commands, skills and rules.", icon: Bot, scoped: false },
    { id: "mcp", label: "MCP", hint: "External tool servers the agent can use.", icon: Server, scoped: true },
    { id: "hooks", label: "Hooks", hint: "Your own commands, run on agent events.", icon: Workflow, scoped: true },
    { id: "advanced", label: "Advanced", hint: "Context, limits, web, shell and language servers.", icon: Sliders, scoped: true },
    { id: "experimental", label: "Experimental", hint: "New features still being measured. Try them in Shadow first.", icon: Lightbulb, scoped: true },
    { id: "about", label: "About & Updates", hint: "Version, updates and where files live.", icon: Info, scoped: false },
  ];
</script>

<script lang="ts">
  import { featureEntries } from "$lib/domain/settings/features";
  import { searchSettings, SETTINGS_SECTIONS, type SettingEntry } from "$lib/domain/settings/searchIndex";
  import { featureStore } from "$lib/stores/features.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SearchField, SelectionCapsule } from "$lib/ui";
  import Icon from "$lib/ui/icons";

  /** Pages grouped by what they are about; searching finds single settings. */
  type Props = {
    tab: SettingsTab;
    version: string | null;
    updateAvailable: boolean;
    onSelect: (tab: SettingsTab) => void;
    onJump: (entry: SettingEntry) => void;
  };
  let { tab, version, updateAvailable, onSelect, onJump }: Props = $props();

  let query = $state("");
  let highlighted = $state(0);
  const results = $derived(searchSettings(query, 8, featureEntries(featureStore.catalog, settingsStore.settings)));
  const byId = new Map(SETTINGS_TABS.map((t) => [t.id, t]));

  function jump(entry: SettingEntry) {
    query = "";
    onJump(entry);
  }

  function onKey(e: KeyboardEvent) {
    if (!results.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      highlighted = (highlighted + step + results.length) % results.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const entry = results[highlighted];
      if (entry) jump(entry);
    }
  }
</script>

<aside class="settings-nav glass" aria-label="Settings pages">
  <SearchField
    bind:value={query}
    label="Search settings"
    placeholder="Search settings"
    oninput={() => (highlighted = 0)}
    onkeydown={onKey}
  />

  <nav class="settings-nav-list">
    {#if query.trim()}
      {#each results as entry, i (entry.key)}
        <button type="button" class="settings-result" class:is-active={i === highlighted} onclick={() => jump(entry)}>
          <span class="settings-result-title">{entry.title}</span>
          <span class="settings-result-tab">{byId.get(entry.tab)?.label}</span>
        </button>
      {:else}
        <p class="settings-results-empty">No setting matches “{query.trim()}”.</p>
      {/each}
    {:else}
      <SelectionCapsule selector={`.settings-nav-item[aria-current="page"]`} />
      {#each SETTINGS_SECTIONS as section (section.label)}
        <section class="settings-nav-section">
          <h3>{section.label}</h3>
          {#each section.tabs as id (id)}
            {@const t = byId.get(id)}
            {#if t}
              <button type="button" class="settings-nav-item" aria-current={tab === id ? "page" : undefined} onclick={() => onSelect(id)}>
                <Icon icon={t.icon} size={15} />
                <span>{t.label}</span>
                {#if id === "about" && updateAvailable}<span class="update-dot" role="status" aria-label="Update available"></span>{/if}
              </button>
            {/if}
          {/each}
        </section>
      {/each}
    {/if}
  </nav>

  <p class="settings-nav-foot">{version ? `HyperForm ${version}` : "HyperForm"}</p>
</aside>

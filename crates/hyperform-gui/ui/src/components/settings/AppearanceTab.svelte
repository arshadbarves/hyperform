<script lang="ts">
  import { untrack } from "svelte";
  import { listExtensions } from "$lib/commands";
  import { APPEARANCE_OPTIONS } from "$lib/domain/appearanceSettings";
  import type { OutputStyleDef } from "$lib/protocol/config/OutputStyleDef";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import AppearancePreview from "./AppearancePreview.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  let styles = $state.raw<OutputStyleDef[] | null>(null);
  let styleError = $state<string | null>(null);
  let saving = $state(false);
  const view = $derived(settings.ui.task_report_view);
  const current = $derived(settings.ui.output_style);
  const selected = $derived(APPEARANCE_OPTIONS.find((option) => option.value === view) ?? APPEARANCE_OPTIONS[0]);
  const missing = $derived(current !== null && styles !== null && !styles.some((style) => style.name === current));

  $effect(() => {
    const root = settingsStore.root;
    untrack(() => {
      listExtensions(root)
        .then((extensions) => (styles = extensions.outputStyles))
        .catch(() => (styles = []));
    });
  });

  async function pickStyle(name: string | null) {
    if (saving || name === current) return;
    saving = true;
    styleError = await settingsStore.setValue(["ui", "output_style"], name);
    saving = false;
  }
</script>

<div class="tab-body appearance-tab">
  <SettingsGroup title="Task report detail" description="Choose how much verification information completed tasks show">
    <SettingsCard>
      <ChoiceSetting
        title="Information density"
        description="This changes report presentation everywhere. It does not change how the agent works or what it verifies."
        keyPath={["ui", "task_report_view"]}
        options={APPEARANCE_OPTIONS}
        value={view}
      />
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup title="Preview" description="An illustration of the layout only — no task or session data is used">
    <AppearancePreview {view} label={selected.label} />
  </SettingsGroup>

  <SettingsGroup title="Response style" description="How the assistant writes its answers. Styles come from output-styles/ folders.">
    <SettingsCard>
      <SettingRow title="Output style" keyPath={["ui", "output_style"]} error={styleError}>
        <div class="style-options" role="radiogroup" aria-label="Output style" aria-busy={saving}>
          <button type="button" role="radio" class="style-option" aria-checked={current === null} onclick={() => void pickStyle(null)}>
            <strong>Default</strong>
            <span>HyperForm's built-in response style.</span>
          </button>
          {#each styles ?? [] as style (style.source.path)}
            <button
              type="button"
              role="radio"
              class="style-option"
              aria-checked={current === style.name}
              onclick={() => void pickStyle(style.name)}
            >
              <strong>{style.name}</strong>
              <span>{style.description || style.source.path}</span>
            </button>
          {/each}
        </div>
        {#if missing}<p class="setting-note">No output style named {current} was found, so the default style is used.</p>{/if}
        {#if styles?.length === 0}
          <p class="setting-note">Add styles under Agents &amp; Commands → Output styles.</p>
        {/if}
      </SettingRow>
    </SettingsCard>
  </SettingsGroup>
</div>

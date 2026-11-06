<script lang="ts">
  import { setTrust } from "$lib/commands";
  import { errorText } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  type Props = { claude: boolean };
  let { claude }: Props = $props();

  const id = $props.id();
  let busy = $state(false);
  let error = $state<string | null>(null);
  const trust = $derived(settingsStore.trust);

  async function toggleTrust() {
    const root = settingsStore.root;
    if (!root || !trust) return;
    busy = true;
    try {
      await setTrust(root, !trust.trusted);
      error = null;
    } catch (e) {
      error = errorText(e);
    }
    await settingsStore.loadTrust();
    busy = false;
  }
</script>

<SettingsGroup title="Workspace" description="Which project files the agent reads and runs.">
  <SettingsCard>
    <ToggleSetting
      title="Read .claude folders"
      description="Also load agents, commands and skills from .claude/ and ~/.claude, and CLAUDE.md instructions."
      keyPath={["compat", "claude"]}
      value={claude}
    />
    {#if settingsStore.root}
      <SettingRow
        title="Trust this workspace"
        description={trust?.projectDefines.length
          ? `Lets this project use its own settings for ${trust.projectDefines.join(", ")}.`
          : "Lets this project's own settings run programs or loosen yours. Its deny and ask rules always apply."}
        controlId={id}
        inline
        {error}
      >
        <label class="switch-toggle">
          <input {id} type="checkbox" checked={trust?.trusted ?? false} disabled={busy || !trust} onchange={() => void toggleTrust()} />
          <span class="switch-slider"></span>
        </label>
      </SettingRow>
    {/if}
  </SettingsCard>
</SettingsGroup>

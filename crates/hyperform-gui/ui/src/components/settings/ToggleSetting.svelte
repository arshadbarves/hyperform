<script lang="ts">
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import SettingRow from "./SettingRow.svelte";

  type Props = { title: string; description?: string; keyPath: KeyPath; value: boolean };
  let { title, description, keyPath, value }: Props = $props();

  const id = $props.id();
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function toggle() {
    saving = true;
    error = await settingsStore.setValue(keyPath, !value);
    saving = false;
  }
</script>

<SettingRow {title} {description} {keyPath} controlId={id} inline {error}>
  <label class="switch-toggle">
    <input {id} type="checkbox" checked={value} disabled={saving} onchange={() => void toggle()} />
    <span class="switch-slider"></span>
  </label>
</SettingRow>

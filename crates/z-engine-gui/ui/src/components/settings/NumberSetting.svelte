<script lang="ts">
  import { parseNumber, type NumberRange } from "$lib/domain/settings/limits";
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import SettingRow from "./SettingRow.svelte";

  type Props = {
    title: string;
    description?: string;
    keyPath: KeyPath;
    value: number | null;
    range: NumberRange;
    /** Blank removes the key. */
    optional?: boolean;
    placeholder?: string;
    unit?: string;
  };

  let { title, description, keyPath, value, range, optional = false, placeholder = "", unit = "" }: Props = $props();

  const id = $props.id();
  let draft = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);

  $effect(() => {
    draft = value === null ? "" : String(value);
  });

  async function commit() {
    const parsed = parseNumber(draft, range, optional);
    if (!parsed.ok) {
      error = parsed.error;
      return;
    }
    if (parsed.value === value) return;
    saving = true;
    error = await settingsStore.setValue(keyPath, parsed.value);
    saving = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      void commit();
    } else if (event.key === "Escape" && draft !== (value === null ? "" : String(value))) {
      event.preventDefault();
      event.stopPropagation();
      draft = value === null ? "" : String(value);
      error = null;
    }
  }
</script>

<SettingRow {title} {description} {keyPath} controlId={id} {error}>
  <div class="setting-number">
    <input
      {id}
      class="setting-input"
      inputmode={range.integer ? "numeric" : "decimal"}
      bind:value={draft}
      {placeholder}
      disabled={saving}
      aria-invalid={error !== null}
      onkeydown={onKeydown}
      oninput={() => (error = null)}
      onblur={() => void commit()}
    />
    {#if unit}<span class="setting-unit">{unit}</span>{/if}
  </div>
</SettingRow>

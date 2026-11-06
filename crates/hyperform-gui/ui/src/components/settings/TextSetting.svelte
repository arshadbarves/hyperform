<script lang="ts">
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import SettingRow from "./SettingRow.svelte";

  type Props = {
    title: string;
    description?: string;
    keyPath: KeyPath;
    value: string | null;
    placeholder?: string;
    /** Blank removes the key instead of being rejected. */
    optional?: boolean;
    mono?: boolean;
    validate?: (text: string) => string | null;
  };

  let { title, description, keyPath, value, placeholder = "", optional = false, mono = false, validate }: Props = $props();

  const id = $props.id();
  let draft = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);

  $effect(() => {
    draft = value ?? "";
  });

  async function commit() {
    const text = draft.trim();
    if (text === (value ?? "")) return;
    const problem = validate?.(text) ?? (!optional && !text ? "Enter a value." : null);
    if (problem) {
      error = problem;
      return;
    }
    saving = true;
    error = await settingsStore.setValue(keyPath, text || null);
    saving = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      void commit();
    } else if (event.key === "Escape" && draft !== (value ?? "")) {
      event.preventDefault();
      event.stopPropagation();
      draft = value ?? "";
      error = null;
    }
  }
</script>

<SettingRow {title} {description} {keyPath} controlId={id} {error}>
  <input
    {id}
    class="setting-input"
    class:mono
    bind:value={draft}
    {placeholder}
    spellcheck={false}
    autocomplete="off"
    disabled={saving}
    aria-invalid={error !== null}
    onkeydown={onKeydown}
    oninput={() => (error = null)}
    onblur={() => void commit()}
  />
</SettingRow>

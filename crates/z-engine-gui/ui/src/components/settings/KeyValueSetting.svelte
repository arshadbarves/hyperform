<script lang="ts">
  import { formatKeyValues, parseKeyValues } from "$lib/domain/settings/formText";
  import { isRecord, type KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import SettingRow from "./SettingRow.svelte";

  /** A string table that merges across layers, such as `shell.env`: the
   * selected file's own entries are edited; other files' keys are listed. */
  type Props = { title: string; description?: string; keyPath: KeyPath; effective: Record<string, string> };
  let { title, description, keyPath, effective }: Props = $props();

  const id = $props.id();
  const own = $derived.by(() => {
    const raw = settingsStore.scopeValue(keyPath);
    return isRecord(raw) ? Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, String(v)])) : {};
  });
  const ownText = $derived(formatKeyValues(own, "="));
  const inherited = $derived(Object.keys(effective).filter((key) => !(key in own)));
  let draft = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    draft = ownText;
  });

  async function save() {
    const parsed = parseKeyValues(draft, "=");
    if (parsed.error) {
      error = parsed.error;
      return;
    }
    saving = true;
    error = await settingsStore.setValue(keyPath, Object.keys(parsed.values).length ? parsed.values : null);
    saving = false;
  }
</script>

<SettingRow {title} {description} controlId={id} {error}>
  <textarea {id} class="setting-textarea mono" rows="3" bind:value={draft} placeholder="NAME=value" spellcheck={false} oninput={() => (error = null)}></textarea>
  {#if inherited.length > 0}
    <p class="setting-note">Also set by other settings files: {inherited.join(", ")}</p>
  {/if}
  {#if draft !== ownText}
    <div class="settings-form-actions">
      <button type="button" class="btn-secondary" onclick={() => (draft = ownText)}>Revert</button>
      <button type="button" class="btn-accent" disabled={saving} onclick={() => void save()}>{saving ? "Saving…" : "Save"}</button>
    </div>
  {/if}
</SettingRow>

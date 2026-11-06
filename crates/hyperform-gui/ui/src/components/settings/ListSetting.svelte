<script lang="ts">
  import { parseList } from "$lib/domain/settings/formText";
  import type { KeyPath, UnionItem } from "$lib/domain/settings/provenance";
  import { LAYER_LABELS, layerOf } from "$lib/domain/settings/scopes";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { Plus, X } from "$lib/ui/icons";
  import SettingRow from "./SettingRow.svelte";

  type Props = {
    title: string;
    description?: string;
    keyPath: KeyPath;
    /** The list the selected layer writes. */
    items: readonly string[];
    /** Union lists: items other layers add, shown read-only. Omitted for lists that override. */
    inherited?: readonly UnionItem[];
    placeholder?: string;
    suggestions?: readonly string[];
    validate?: (item: string) => string | null;
    mono?: boolean;
  };

  let { title, description, keyPath, items, inherited, placeholder = "", suggestions = [], validate, mono = true }: Props =
    $props();

  const id = $props.id();
  let draft = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);
  const union = $derived(inherited !== undefined);
  const missing = $derived(suggestions.filter((item) => !items.includes(item)));

  async function save(next: string[]) {
    saving = true;
    error = await settingsStore.setValue(keyPath, next);
    saving = false;
    return error === null;
  }

  async function add(text: string) {
    const added = parseList(text).filter((item) => !items.includes(item));
    const problem = added.map((item) => validate?.(item) ?? null).find(Boolean);
    if (problem) {
      error = problem;
      return;
    }
    if (added.length > 0 && (await save([...items, ...added])) && text === draft) draft = "";
  }
</script>

<SettingRow {title} {description} keyPath={union ? undefined : keyPath} controlId={id} {error}>
  <div class="setting-list">
    {#if items.length === 0 && !inherited?.length}
      <span class="setting-list-empty">None</span>
    {/if}
    {#each items as item (item)}
      <span class="setting-chip" class:mono>
        <span class="setting-chip-text">{item}</span>
        <button
          type="button"
          class="setting-chip-remove"
          disabled={saving}
          aria-label={`Remove ${item}`}
          onclick={() => void save(items.filter((other) => other !== item))}
        >
          <Icon icon={X} size={10} />
        </button>
      </span>
    {/each}
    {#each inherited ?? [] as item (item.value)}
      <span class="setting-chip inherited" class:mono title={`From ${item.scopes.map((s) => LAYER_LABELS[layerOf(s)]).join(", ")}`}>
        <span class="setting-chip-text">{item.value}</span>
        <span class="setting-chip-source">{item.scopes.map((s) => LAYER_LABELS[layerOf(s)]).join(", ")}</span>
      </span>
    {/each}
  </div>
  <form
    class="setting-add-row"
    onsubmit={(event) => {
      event.preventDefault();
      void add(draft);
    }}
  >
    <input {id} class="setting-input" class:mono bind:value={draft} {placeholder} spellcheck={false} autocomplete="off" oninput={() => (error = null)} />
    <button type="submit" class="btn-secondary" disabled={!draft.trim() || saving}>
      <Icon icon={Plus} size={12} />
      <span>Add</span>
    </button>
  </form>
  {#if missing.length > 0}
    <div class="setting-suggestions">
      {#each missing as item (item)}
        <button type="button" class="preset-chip-btn" disabled={saving} onclick={() => void add(item)}>
          <Icon icon={Plus} size={11} />
          <code>{item}</code>
        </button>
      {/each}
    </div>
  {/if}
</SettingRow>

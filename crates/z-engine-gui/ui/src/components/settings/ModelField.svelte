<script lang="ts">
  import { catalogForPicker, catalogStore, fmtLimit, groupModels, isFreeModel } from "$lib/catalog";
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { Pill } from "$lib/ui";
  import Icon, { Brain } from "$lib/ui/icons";
  import SettingRow from "./SettingRow.svelte";

  const SUGGESTIONS = 8;

  type Props = {
    title: string;
    description?: string;
    keyPath: KeyPath;
    value: string | null;
    /** Blank removes the key (the model falls back to the main model). */
    optional?: boolean;
    placeholder?: string;
  };

  let { title, description, keyPath, value, optional = false, placeholder = "" }: Props = $props();

  const id = $props.id();
  const catalog = bindStore(catalogStore);
  let draft = $state("");
  let open = $state(false);
  let highlighted = $state(0);
  let error = $state<string | null>(null);
  let saving = $state(false);

  $effect(() => {
    draft = value ?? "";
  });

  const provider = $derived(settingsStore.activeProvider);
  const models = $derived(catalogForPicker(catalog.current, provider?.baseUrl, provider?.hasKey ?? false));
  const query = $derived(draft.trim() === (value ?? "") ? "" : draft);
  const matches = $derived(
    groupModels(models, query, SUGGESTIONS)
      .flatMap((group) => group.items)
      .slice(0, SUGGESTIONS),
  );
  const listOpen = $derived(open && matches.length > 0);

  async function commit(next: string) {
    open = false;
    if (saving) return;
    const text = next.trim();
    draft = text;
    if (text === (value ?? "")) return;
    if (!text && !optional) {
      error = "Enter a model id.";
      return;
    }
    saving = true;
    error = await settingsStore.setValue(keyPath, text || null);
    saving = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      open = true;
      const step = event.key === "ArrowDown" ? 1 : -1;
      highlighted = matches.length ? (highlighted + step + matches.length) % matches.length : 0;
    } else if (event.key === "Enter") {
      event.preventDefault();
      void commit(listOpen ? (matches[highlighted]?.id ?? draft) : draft);
    } else if (event.key === "Escape" && (listOpen || draft !== (value ?? ""))) {
      event.preventDefault();
      event.stopPropagation();
      if (listOpen) open = false;
      else draft = value ?? "";
    }
  }
</script>

<SettingRow {title} {description} {keyPath} controlId={id} {error}>
  <div class="model-field">
    <input
      {id}
      class="setting-input mono"
      role="combobox"
      aria-expanded={listOpen}
      aria-controls={`${id}-list`}
      aria-autocomplete="list"
      aria-activedescendant={listOpen ? `${id}-${highlighted}` : undefined}
      bind:value={draft}
      {placeholder}
      spellcheck={false}
      autocomplete="off"
      disabled={saving}
      onfocus={() => {
        open = true;
        void catalogStore.ensure();
      }}
      onclick={() => (open = true)}
      oninput={() => {
        open = true;
        highlighted = 0;
        error = null;
      }}
      onkeydown={onKeydown}
      onblur={() => void commit(draft)}
    />
    {#if listOpen}
      <div class="model-field-list" id={`${id}-list`} role="listbox" aria-label={`${title} suggestions`}>
        {#each matches as model, index (model.id)}
          <div
            id={`${id}-${index}`}
            class={`model-picker-row${index === highlighted ? " active" : ""}`}
            role="option"
            tabindex="-1"
            aria-selected={index === highlighted}
            onmousedown={(event) => {
              event.preventDefault();
              void commit(model.id);
            }}
          >
            <div class="model-row-left">
              <div class="model-row-name-line">
                <span class="model-row-name">{model.name}</span>
                {#if isFreeModel(model)}<Pill tone="ok">Free</Pill>{/if}
                {#if model.reasoning}
                  <span class="model-chip-reasoning"><Icon icon={Brain} size={9} /><span>Reasoning</span></span>
                {/if}
              </div>
              <div class="model-row-sub"><span class="model-row-id">{model.id}</span></div>
            </div>
            {#if model.contextWindow || model.maxOutput}
              <span class="model-chip-spec">
                {[fmtLimit(model.contextWindow), fmtLimit(model.maxOutput)].filter(Boolean).join(" / ")}
              </span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</SettingRow>

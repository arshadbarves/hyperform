<script lang="ts">
  import { EXTENSION_SCOPE_LABELS, isNativeScope, type ExtensionEntry, type ExtensionKindMeta } from "$lib/domain/settings/extensions";
  import { Pill } from "$lib/ui";
  import Icon, { Copy, Pencil, Plus, Trash2 } from "$lib/ui/icons";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  type Props = {
    meta: ExtensionKindMeta;
    entries: readonly ExtensionEntry[];
    busy: boolean;
    onCreate: () => void;
    onEdit: (entry: ExtensionEntry) => void;
    onDelete: (entry: ExtensionEntry) => void;
  };

  let { meta, entries, busy, onCreate, onEdit, onDelete }: Props = $props();

  let confirming = $state<string | null>(null);
</script>

<SettingsGroup title={meta.label} description={meta.description}>
  <SettingsCard>
    {#if entries.length === 0}
      <div class="extension-empty">No {meta.label.toLowerCase()} yet.</div>
    {/if}
    {#each entries as entry (entry.source.path)}
      {@const native = isNativeScope(entry.source.scope)}
      <div class="extension-row">
        <div class="extension-row-copy">
          <div class="extension-row-title">
            <strong>{meta.kind === "commands" ? `/${entry.name}` : entry.name}</strong>
            <Pill>{EXTENSION_SCOPE_LABELS[entry.source.scope]}</Pill>
          </div>
          {#if entry.description}<span class="extension-row-desc">{entry.description}</span>{/if}
          <code class="extension-row-path" title={entry.source.path}>{entry.source.path}</code>
        </div>
        <div class="extension-row-actions">
          {#if confirming === entry.source.path}
            <button type="button" class="btn-secondary size-s" onclick={() => (confirming = null)}>Keep</button>
            <button type="button" class="btn-danger size-s" disabled={busy} onclick={() => onDelete(entry)}>Delete file</button>
          {:else}
            <button
              type="button"
              class="icon-btn-mini"
              disabled={busy || entry.fileName === null}
              title={native ? "Edit" : "Copy into HyperForm's folder to edit; the copy takes precedence"}
              aria-label={native ? `Edit ${entry.name}` : `Copy ${entry.name} to edit`}
              onclick={() => onEdit(entry)}
            >
              <Icon icon={native ? Pencil : Copy} size={12} />
            </button>
            {#if native}
              <button
                type="button"
                class="icon-btn-mini setting-remove"
                disabled={busy}
                aria-label={`Delete ${entry.name}`}
                onclick={() => (confirming = entry.source.path)}
              >
                <Icon icon={Trash2} size={12} />
              </button>
            {/if}
          {/if}
        </div>
      </div>
    {/each}
    <div class="extension-foot">
      <button type="button" class="btn-secondary" disabled={busy} onclick={onCreate}>
        <Icon icon={Plus} size={12} />
        <span>New {meta.singular}</span>
      </button>
    </div>
  </SettingsCard>
</SettingsGroup>

<script lang="ts">
  import { CHECKS_PATH, checkForFile, emptyCheck, layerChecks } from "$lib/domain/settings/checks";
  import { appendItem, removeItem, replaceItem } from "$lib/domain/settings/listEdits";
  import { idOverride } from "$lib/domain/settings/provenance";
  import { LAYER_LABELS, layerOf } from "$lib/domain/settings/scopes";
  import type { CheckConfig } from "$lib/protocol/config/CheckConfig";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";
  import Icon, { Copy, Pencil, Plus, Trash2 } from "$lib/ui/icons";
  import CheckForm from "./CheckForm.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SourceBadge from "./SourceBadge.svelte";

  /** An index into the edited file's checks, or a new check (possibly copied). */
  let editing = $state.raw<{ index: number | null; check: CheckConfig } | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const layers = $derived(
    settingsStore.scopes.map((scope) => ({
      scope,
      checks: layerChecks(settingsStore.files[scope]?.raw).map((check) => ({
        check,
        overriddenBy: settingsStore.provenance ? idOverride(settingsStore.provenance, scope, CHECKS_PATH, check.id) : null,
      })),
    })),
  );
  const own = $derived(layerChecks(settingsStore.files[settingsStore.scope]?.raw));
  const total = $derived(layers.reduce((sum, layer) => sum + layer.checks.length, 0));

  async function save(list: CheckConfig[]): Promise<boolean> {
    busy = true;
    error = await settingsStore.setValue(CHECKS_PATH, list.length ? list.map(checkForFile) : null);
    busy = false;
    return error === null;
  }

  function siblings(index: number | null): CheckConfig[] {
    return index === null ? own : removeItem(own, index);
  }
</script>

<SettingsGroup
  title="Checks"
  description="Commands the engine runs as evidence. A check in a higher file replaces one with the same id."
>
  <SettingsCard>
    {#if total === 0}
      <div class="extension-empty">
        No checks configured. Checks discovered from the project's manifests (Cargo.toml, package.json, …) still apply.
      </div>
    {/if}
    {#each layers as layer (layer.scope)}
      {#each layer.checks as entry, index (`${layer.scope}-${entry.check.id}-${index}`)}
        {@const mine = layer.scope === settingsStore.scope}
        {#if mine && editing?.index === index}
          <CheckForm check={entry.check} siblings={siblings(index)} onSave={(next) => save(replaceItem(own, index, next))} onCancel={() => (editing = null)} />
        {:else}
          <div class="entry-row" class:is-overridden={entry.overriddenBy !== null}>
            <div class="entry-row-copy">
              <div class="entry-row-title">
                <strong>{entry.check.label || entry.check.id}</strong>
                <Pill>{entry.check.kind}</Pill>
                {#if entry.check.label}<code class="entry-id">{entry.check.id}</code>{/if}
                {#if entry.overriddenBy}<span class="hook-skipped">replaced by {LAYER_LABELS[layerOf(entry.overriddenBy)]}</span>{/if}
              </div>
              <code class="entry-command" title={entry.check.command}>
                {entry.check.cwd ? `(${entry.check.cwd}) ` : ""}{entry.check.command}
              </code>
            </div>
            <div class="hook-row-meta">
              <SourceBadge source={layerOf(layer.scope)} />
              {#if mine}
                <button type="button" class="icon-btn-mini" disabled={busy || editing !== null} aria-label={`Edit ${entry.check.id}`} onclick={() => (editing = { index, check: entry.check })}>
                  <Icon icon={Pencil} size={11} />
                </button>
                <button type="button" class="icon-btn-mini setting-remove" disabled={busy} aria-label={`Remove ${entry.check.id}`} onclick={() => void save(removeItem(own, index))}>
                  <Icon icon={Trash2} size={12} />
                </button>
              {:else}
                <button type="button" class="icon-btn-mini" disabled={busy || editing !== null} title="Copy to the settings file being edited" aria-label={`Copy ${entry.check.id}`} onclick={() => (editing = { index: null, check: entry.check })}>
                  <Icon icon={Copy} size={11} />
                </button>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
    {/each}
    {#if editing && editing.index === null}
      <CheckForm check={editing.check} siblings={own} onSave={(next) => save(appendItem(own, next))} onCancel={() => (editing = null)} />
    {:else}
      <div class="extension-foot">
        <button type="button" class="btn-secondary" disabled={busy || editing !== null} onclick={() => (editing = { index: null, check: emptyCheck() })}>
          <Icon icon={Plus} size={12} />
          <span>Add check</span>
        </button>
      </div>
    {/if}
    {#if error}<p class="setting-error mcp-error" role="alert">{error}</p>{/if}
  </SettingsCard>
</SettingsGroup>

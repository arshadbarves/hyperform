<script lang="ts">
  import { formatArgs } from "$lib/domain/settings/formText";
  import { emptyLspForm, LSP_SERVERS_PATH, lspFormFrom, normalizeLspServer, type LspForm } from "$lib/domain/settings/lspForm";
  import { namedOverride, rawAt, tableNames } from "$lib/domain/settings/provenance";
  import { LAYER_LABELS, layerOf } from "$lib/domain/settings/scopes";
  import { settingWrite } from "$lib/domain/settings/tomlValue";
  import type { LspServerConfig } from "$lib/protocol/config/LspServerConfig";
  import type { LspSettings } from "$lib/protocol/config/LspSettings";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";
  import Icon, { Copy, Pencil, Plus, Trash2 } from "$lib/ui/icons";
  import LspServerForm from "./LspServerForm.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SourceBadge from "./SourceBadge.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  type Props = { lsp: LspSettings };
  let { lsp }: Props = $props();

  let editing = $state.raw<{ original: string | null; form: LspForm } | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const rows = $derived(
    settingsStore.scopes.flatMap((scope) => {
      const raw = settingsStore.files[scope]?.raw;
      const provenance = settingsStore.provenance;
      return tableNames(raw, LSP_SERVERS_PATH).map((name) => ({
        scope,
        name,
        server: normalizeLspServer(rawAt(raw, [...LSP_SERVERS_PATH, name])),
        overriddenBy: provenance ? namedOverride(provenance, scope, LSP_SERVERS_PATH, name) : null,
      }));
    }),
  );
  const ownNames = $derived(rows.filter((row) => row.scope === settingsStore.scope).map((row) => row.name));

  async function save(name: string, server: LspServerConfig): Promise<boolean> {
    const original = editing?.original ?? null;
    const writes = [settingWrite([...LSP_SERVERS_PATH, name], server)];
    if (original && original !== name) writes.push(settingWrite([...LSP_SERVERS_PATH, original], null));
    busy = true;
    error = await settingsStore.apply(writes);
    busy = false;
    return error === null;
  }

  async function remove(name: string) {
    busy = true;
    error = await settingsStore.reset([...LSP_SERVERS_PATH, name]);
    busy = false;
  }
</script>

<SettingsGroup title="Language servers" description="Diagnostics and code navigation. A server in a higher file replaces one with the same name.">
  <SettingsCard>
    <ToggleSetting title="Use language servers" keyPath={["lsp", "enabled"]} value={lsp.enabled} />
    {#each rows as row (`${row.scope}:${row.name}`)}
      {@const mine = row.scope === settingsStore.scope}
      {#if mine && editing?.original === row.name}
        <LspServerForm form={editing.form} taken={ownNames.filter((n) => n !== row.name)} onSave={save} onCancel={() => (editing = null)} />
      {:else}
        <div class="entry-row" class:is-overridden={row.overriddenBy !== null}>
          <div class="entry-row-copy">
            <div class="entry-row-title">
              <strong>{row.name}</strong>
              <Pill>{row.server.extensions.map((ext) => `.${ext}`).join(" ")}</Pill>
              {#if !row.server.enabled}<Pill>Disabled</Pill>{/if}
              {#if row.overriddenBy}<span class="hook-skipped">replaced by {LAYER_LABELS[layerOf(row.overriddenBy)]}</span>{/if}
            </div>
            <code class="entry-command">{[row.server.command, formatArgs(row.server.args)].filter(Boolean).join(" ")}</code>
          </div>
          <div class="hook-row-meta">
            <SourceBadge source={layerOf(row.scope)} />
            {#if mine}
              <button type="button" class="icon-btn-mini" disabled={busy || editing !== null} aria-label={`Edit ${row.name}`} onclick={() => (editing = { original: row.name, form: lspFormFrom(row.name, row.server) })}>
                <Icon icon={Pencil} size={11} />
              </button>
              <button type="button" class="icon-btn-mini setting-remove" disabled={busy} aria-label={`Remove ${row.name}`} onclick={() => void remove(row.name)}>
                <Icon icon={Trash2} size={12} />
              </button>
            {:else}
              <button type="button" class="icon-btn-mini" disabled={busy || editing !== null} title="Copy to the settings file being edited" aria-label={`Copy ${row.name}`} onclick={() => (editing = { original: null, form: lspFormFrom(row.name, row.server) })}>
                <Icon icon={Copy} size={11} />
              </button>
            {/if}
          </div>
        </div>
      {/if}
    {/each}
    {#if editing && editing.original === null}
      <LspServerForm form={editing.form} taken={ownNames} onSave={save} onCancel={() => (editing = null)} />
    {:else}
      <div class="extension-foot">
        <button type="button" class="btn-secondary" disabled={busy || editing !== null} onclick={() => (editing = { original: null, form: emptyLspForm() })}>
          <Icon icon={Plus} size={12} />
          <span>Add language server</span>
        </button>
      </div>
    {/if}
    {#if error}<p class="setting-error mcp-error" role="alert">{error}</p>{/if}
  </SettingsCard>
</SettingsGroup>

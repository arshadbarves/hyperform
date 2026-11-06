<script lang="ts">
  import { removeMcpServer, setMcpServer, testMcpServer } from "$lib/commands";
  import { emptyMcpForm, MCP_SERVERS_PATH, mcpFormFrom, normalizeMcpServer, type McpForm } from "$lib/domain/settings/mcpForm";
  import { namedOverride, rawAt, tableNames } from "$lib/domain/settings/provenance";
  import { scopeLabel, type SettingsScope } from "$lib/domain/settings/scopes";
  import type { McpServerConfig } from "$lib/protocol/config/McpServerConfig";
  import { errorText } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { Plus } from "$lib/ui/icons";
  import McpServerForm from "./McpServerForm.svelte";
  import McpServerRow, { type TestState } from "./McpServerRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TrustNote from "./TrustNote.svelte";

  /** `original` is the name being edited in the selected file; null adds a server. */
  let editing = $state.raw<{ original: string | null; form: McpForm } | null>(null);
  let tests = $state<Record<string, TestState>>({});
  let busy = $state(false);
  let error = $state<string | null>(null);

  const sections = $derived(
    settingsStore.scopes.map((scope) => {
      const raw = settingsStore.files[scope]?.raw;
      const provenance = settingsStore.provenance;
      const servers = tableNames(raw, MCP_SERVERS_PATH).map((name) => ({
        name,
        server: normalizeMcpServer(rawAt(raw, [...MCP_SERVERS_PATH, name])),
        overriddenBy: provenance ? namedOverride(provenance, scope, MCP_SERVERS_PATH, name) : null,
      }));
      return { scope, servers };
    }),
  );
  const ownNames = $derived(sections.find((s) => s.scope === settingsStore.scope)?.servers.map((s) => s.name) ?? []);

  async function run(op: Parameters<typeof settingsStore.write>[0]): Promise<boolean> {
    busy = true;
    error = await settingsStore.write(op);
    busy = false;
    return error === null;
  }

  function save(name: string, server: McpServerConfig): Promise<boolean> {
    const original = editing?.original ?? null;
    return run(async (target) => {
      await setMcpServer(target, name, server);
      if (original && original !== name) await removeMcpServer(target, original);
    });
  }

  async function test(key: string, server: McpServerConfig) {
    tests = { ...tests, [key]: { running: true } };
    try {
      const result = await testMcpServer(server, settingsStore.root);
      tests = { ...tests, [key]: { running: false, result } };
    } catch (e) {
      tests = { ...tests, [key]: { running: false, error: errorText(e) } };
    }
  }

  function open(scope: SettingsScope, name: string, server: McpServerConfig) {
    const own = scope === settingsStore.scope;
    editing = { original: own ? name : null, form: mcpFormFrom(name, server) };
  }
</script>

<div class="tab-body mcp-tab">
  <p class="form-note">
    Servers from every settings file are loaded; a server in a higher file replaces one with the same name. Their tools
    appear to the model as <code>mcp__server__tool</code>, and permission rules can target them the same way.
  </p>

  <TrustNote what="MCP servers" />

  {#each sections as section (section.scope)}
    {@const mine = section.scope === settingsStore.scope}
    {#if mine || section.servers.length > 0}
      <SettingsGroup
        title={`${scopeLabel(section.scope)} servers`}
        description={mine ? "The settings file being edited." : "Read-only here; copy a server to override it."}
      >
        <SettingsCard>
          {#if section.servers.length === 0}
            <div class="extension-empty">No servers in this file.</div>
          {/if}
          <div class="mcp-servers-list">
            {#each section.servers as entry (entry.name)}
              {@const key = `${section.scope}:${entry.name}`}
              {#if mine && editing?.original === entry.name}
                <McpServerForm form={editing.form} taken={ownNames.filter((n) => n !== entry.name)} onSave={save} onCancel={() => (editing = null)} />
              {:else}
                <McpServerRow
                  name={entry.name}
                  server={entry.server}
                  editable={mine}
                  overriddenBy={entry.overriddenBy}
                  test={tests[key]}
                  busy={busy || editing !== null}
                  onTest={() => void test(key, entry.server)}
                  onEdit={() => open(section.scope, entry.name, entry.server)}
                  onRemove={() => void run((target) => removeMcpServer(target, entry.name))}
                  onToggle={() => void run((target) => setMcpServer(target, entry.name, { ...entry.server, enabled: !entry.server.enabled }))}
                />
              {/if}
            {/each}
          </div>
          {#if mine}
            {#if editing && editing.original === null}
              <McpServerForm form={editing.form} taken={ownNames} onSave={save} onCancel={() => (editing = null)} />
            {:else}
              <div class="extension-foot">
                <button type="button" class="btn-secondary" disabled={busy || editing !== null} onclick={() => (editing = { original: null, form: emptyMcpForm() })}>
                  <Icon icon={Plus} size={12} />
                  <span>Add server</span>
                </button>
              </div>
            {/if}
            {#if error}<p class="setting-error mcp-error" role="alert">{error}</p>{/if}
          {/if}
        </SettingsCard>
      </SettingsGroup>
    {/if}
  {/each}
</div>

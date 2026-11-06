<script lang="ts">
  import { untrack } from "svelte";
  import { mcpNameError, mcpServerFrom, type McpForm, type McpTransport } from "$lib/domain/settings/mcpForm";
  import type { McpServerConfig } from "$lib/protocol/config/McpServerConfig";
  import { SegmentedChoice } from "$lib/ui";
  import { cancelOnEscape } from "./escape";

  type Props = {
    form: McpForm;
    /** Other server names in the file being edited. */
    taken: readonly string[];
    /** Resolves to true once saved. */
    onSave: (name: string, server: McpServerConfig) => Promise<boolean>;
    onCancel: () => void;
  };

  let { form: initialForm, taken, onSave, onCancel }: Props = $props();

  const TRANSPORTS: ReadonlyArray<{ value: McpTransport; label: string; description: string }> = [
    { value: "stdio", label: "Command (stdio)", description: "Starts a local process and talks over stdin/stdout." },
    { value: "http", label: "URL (HTTP)", description: "Connects to a streamable HTTP endpoint." },
  ];

  const id = $props.id();
  const form = $state<McpForm>({ ...untrack(() => initialForm) });
  let saving = $state(false);
  let touched = $state(false);

  const built = $derived(mcpServerFrom(form));
  const problem = $derived(mcpNameError(form.name, taken) ?? ("error" in built ? built.error : null));

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    touched = true;
    if (problem || !("server" in built)) return;
    saving = true;
    const saved = await onSave(form.name.trim(), built.server);
    saving = false;
    if (saved) onCancel();
  }
</script>

<form class="settings-form mcp-form" onsubmit={submit} {@attach cancelOnEscape(onCancel)}>
  <label class="settings-form-field" for={`${id}-name`}>
    <span>Name</span>
    <input id={`${id}-name`} class="setting-input mono" bind:value={form.name} placeholder="github" spellcheck={false} />
  </label>
  <div class="settings-form-field">
    <span>Transport</span>
    <SegmentedChoice label="Transport" options={TRANSPORTS} value={form.transport} onSelect={(next) => (form.transport = next)} />
  </div>
  {#if form.transport === "stdio"}
    <label class="settings-form-field" for={`${id}-command`}>
      <span>Command</span>
      <input id={`${id}-command`} class="setting-input mono" bind:value={form.command} placeholder="npx" spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-args`}>
      <span>Arguments</span>
      <input id={`${id}-args`} class="setting-input mono" bind:value={form.args} placeholder="-y @modelcontextprotocol/server-filesystem ." spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-env`}>
      <span>Environment (NAME=value per line)</span>
      <textarea id={`${id}-env`} class="setting-textarea mono" rows="2" bind:value={form.env} spellcheck={false}></textarea>
    </label>
    <label class="settings-form-field" for={`${id}-cwd`}>
      <span>Working directory</span>
      <input id={`${id}-cwd`} class="setting-input mono" bind:value={form.cwd} placeholder="Blank uses the project root" spellcheck={false} />
    </label>
  {:else}
    <label class="settings-form-field" for={`${id}-url`}>
      <span>URL</span>
      <input id={`${id}-url`} class="setting-input mono" bind:value={form.url} placeholder="https://mcp.example.com/mcp" spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-headers`}>
      <span>Headers (Name: value per line)</span>
      <textarea id={`${id}-headers`} class="setting-textarea mono" rows="2" bind:value={form.headers} spellcheck={false}></textarea>
    </label>
  {/if}
  <label class="settings-form-field" for={`${id}-disabled`}>
    <span>Hidden tools (comma separated)</span>
    <input id={`${id}-disabled`} class="setting-input mono" bind:value={form.disabledTools} placeholder="delete_file, move_file" spellcheck={false} />
  </label>
  <div class="settings-form-row">
    <label class="settings-form-field narrow" for={`${id}-timeout`}>
      <span>Timeout (seconds)</span>
      <input id={`${id}-timeout`} class="setting-input" inputmode="numeric" bind:value={form.timeout} />
    </label>
    <label class="settings-form-check">
      <input type="checkbox" bind:checked={form.enabled} />
      <span>Enabled</span>
    </label>
  </div>
  {#if touched && problem}<p class="setting-error" role="alert">{problem}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || (touched && problem !== null)}>
      {saving ? "Saving…" : "Save server"}
    </button>
  </div>
</form>

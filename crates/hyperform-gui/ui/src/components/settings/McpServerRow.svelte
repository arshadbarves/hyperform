<script module lang="ts">
  import type { McpTestResult } from "$lib/commands";

  export type TestState = { running: true } | { running: false; result: McpTestResult } | { running: false; error: string };
</script>

<script lang="ts">
  import { serverSummary } from "$lib/domain/settings/mcpForm";
  import { LAYER_LABELS, layerOf, type SettingsScope } from "$lib/domain/settings/scopes";
  import type { McpServerConfig } from "$lib/protocol/config/McpServerConfig";
  import { Pill } from "$lib/ui";
  import Icon, { AlertTriangle, CheckCircle2, Copy, LoaderCircle, Pencil, RefreshCw, Server, Trash2 } from "$lib/ui/icons";

  type Props = {
    name: string;
    server: McpServerConfig;
    /** The row belongs to the settings file being edited. */
    editable: boolean;
    overriddenBy: SettingsScope | null;
    test: TestState | undefined;
    busy: boolean;
    onTest: () => void;
    onEdit: () => void;
    onRemove: () => void;
    onToggle: () => void;
  };

  let { name, server, editable, overriddenBy, test, busy, onTest, onEdit, onRemove, onToggle }: Props = $props();

  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
</script>

<div class="mcp-server-item" class:is-disabled={!server.enabled} class:is-overridden={overriddenBy !== null}>
  <div class="mcp-server-main">
    <div class="mcp-server-info">
      <div class="mcp-title-row">
        <span class="mcp-server-badge"><Icon icon={Server} size={12} /></span>
        <strong class="mcp-server-name">{name}</strong>
        <Pill>{server.url ? "HTTP" : "stdio"}</Pill>
        {#if !server.enabled}<Pill>Disabled</Pill>{/if}
        {#if overriddenBy}<span class="hook-skipped">replaced by {LAYER_LABELS[layerOf(overriddenBy)]}</span>{/if}
      </div>
      <code class="mcp-server-cmd" title={serverSummary(server)}>{serverSummary(server)}</code>
    </div>
    <div class="mcp-server-actions">
      <button type="button" class="btn-secondary size-s" disabled={test?.running} onclick={onTest} title={`Start ${name} and list what it offers`}>
        <Icon icon={test?.running ? LoaderCircle : RefreshCw} size={12} class={test?.running ? "spin" : undefined} />
        <span>{test?.running ? "Testing…" : "Test"}</span>
      </button>
      {#if editable}
        <label class="switch-toggle" title={server.enabled ? "Disable" : "Enable"}>
          <input type="checkbox" checked={server.enabled} disabled={busy} onchange={onToggle} aria-label={`Enable ${name}`} />
          <span class="switch-slider"></span>
        </label>
        <button type="button" class="icon-btn-mini" disabled={busy} aria-label={`Edit ${name}`} onclick={onEdit}>
          <Icon icon={Pencil} size={12} />
        </button>
        <button type="button" class="icon-btn-mini setting-remove" disabled={busy} aria-label={`Remove ${name}`} onclick={onRemove}>
          <Icon icon={Trash2} size={12} />
        </button>
      {:else}
        <button type="button" class="icon-btn-mini" disabled={busy} title="Copy to the settings file being edited" aria-label={`Copy ${name}`} onclick={onEdit}>
          <Icon icon={Copy} size={12} />
        </button>
      {/if}
    </div>
  </div>
  {#if test && !test.running}
    {#if "result" in test && test.result.ok}
      <div class="mcp-result-banner ok">
        <Icon icon={CheckCircle2} size={13} />
        <span>
          {plural(test.result.tools.length, "tool")} · {plural(test.result.resources, "resource")} · {plural(test.result.prompts, "prompt")}
          {#if test.result.tools.length}— {test.result.tools.join(", ")}{/if}
        </span>
      </div>
    {:else}
      <div class="mcp-result-banner err">
        <Icon icon={AlertTriangle} size={13} />
        <span>{"error" in test ? test.error : (test.result.error ?? "The server did not start.")}</span>
      </div>
    {/if}
  {/if}
</div>

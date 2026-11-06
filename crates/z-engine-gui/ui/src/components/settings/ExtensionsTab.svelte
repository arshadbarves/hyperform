<script lang="ts">
  import { untrack } from "svelte";
  import { deleteExtensionFile, listExtensions, readExtensionFile } from "$lib/commands";
  import { markdownFromDef } from "$lib/domain/settings/extensionMarkdown";
  import { EXTENSION_KINDS, extensionEntries, isNativeScope, type ExtensionEntry, type ExtensionKind } from "$lib/domain/settings/extensions";
  import type { Extensions } from "$lib/protocol/config/Extensions";
  import { catalogs, errorText, pushToast } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { AlertTriangle } from "$lib/ui/icons";
  import ExtensionEditor, { type ExtensionDraft } from "./ExtensionEditor.svelte";
  import ExtensionSection from "./ExtensionSection.svelte";

  let extensions = $state.raw<Extensions | null>(null);
  let loadError = $state<string | null>(null);
  let draft = $state.raw<ExtensionDraft | null>(null);
  let busy = $state(false);
  const root = $derived(settingsStore.root);

  async function load() {
    try {
      extensions = await listExtensions(settingsStore.root);
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void root;
    untrack(() => void load());
  });

  function defOf(kind: ExtensionKind, path: string): unknown {
    const lists = extensions && {
      agents: extensions.agents,
      commands: extensions.commands,
      skills: extensions.skills,
      rules: extensions.rules,
      "output-styles": extensions.outputStyles,
    };
    return lists?.[kind].find((def) => def.source.path === path);
  }

  function create(kind: ExtensionKind) {
    draft = { kind, creating: true, name: "", location: root ? "project" : "user", content: "", notice: null, blocked: false };
  }

  async function edit(entry: ExtensionEntry) {
    const native = isNativeScope(entry.source.scope);
    const location = entry.source.scope === "project" || entry.source.scope === "claudeProject" ? "project" : "user";
    let content: string;
    let notice: string | null = native ? null : "A copy in HyperForm's folder replaces the .claude file of the same name.";
    let blocked = false;
    try {
      content = await readExtensionFile(entry.source.path);
    } catch {
      content = markdownFromDef(entry.kind, defOf(entry.kind, entry.source.path));
      blocked = entry.kind === "skills";
      notice = blocked
        ? "The skill's instructions could not be loaded, so saving is disabled to keep SKILL.md intact."
        : "Rebuilt from the parsed file: comments and unrecognised frontmatter keys are not kept.";
    }
    draft = { kind: entry.kind, creating: !native, name: entry.fileName ?? entry.name, location, content, notice, blocked };
  }

  async function afterChange() {
    await load();
    await catalogs.reload(root);
  }

  async function saved(path: string) {
    draft = null;
    pushToast(`Saved ${path}`, "ok");
    await afterChange();
  }

  async function remove(entry: ExtensionEntry) {
    busy = true;
    try {
      await deleteExtensionFile(entry.source.path);
      pushToast(`Deleted ${entry.name}`, "info");
    } catch (e) {
      pushToast(`Could not delete ${entry.name} · ${errorText(e)}`, "warn");
    }
    await afterChange();
    busy = false;
  }
</script>

<div class="tab-body extensions-tab">
  {#if loadError}
    <div class="settings-notice error"><Icon icon={AlertTriangle} size={13} /><span>Could not list extensions: {loadError}</span></div>
  {/if}
  {#if extensions?.errors.length}
    <div class="settings-notices">
      {#each extensions.errors as problem (problem.path)}
        <div class="settings-notice warn">
          <Icon icon={AlertTriangle} size={13} />
          <span><code>{problem.path}</code> was skipped: {problem.message}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if draft}
    {#key draft}
      <ExtensionEditor {draft} projectRoot={root} onSaved={(path) => void saved(path)} onCancel={() => (draft = null)} />
    {/key}
  {/if}

  {#if extensions}
    {#each EXTENSION_KINDS as meta (meta.kind)}
      <ExtensionSection
        {meta}
        entries={extensionEntries(extensions, meta.kind)}
        busy={busy || draft !== null}
        onCreate={() => create(meta.kind)}
        onEdit={(entry) => void edit(entry)}
        onDelete={(entry) => void remove(entry)}
      />
    {/each}
  {:else if !loadError}
    <div class="settings-loading">Loading extensions…</div>
  {/if}
</div>

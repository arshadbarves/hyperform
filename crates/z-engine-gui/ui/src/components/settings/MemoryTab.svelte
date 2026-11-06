<script lang="ts">
  import { untrack } from "svelte";
  import { listInstructionFiles } from "$lib/commands";
  import { instructionSlots, otherInstructionFiles } from "$lib/domain/settings/instructions";
  import type { InstructionFile } from "$lib/protocol/config/InstructionFile";
  import { errorText, pushToast } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";
  import Icon, { AlertTriangle, FileText, Pencil, Plus } from "$lib/ui/icons";
  import InstructionEditor from "./InstructionEditor.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  const SCOPE_LABELS = { user: "User", project: "Project", local: "Personal", nested: "Nested" } as const;

  let files = $state.raw<InstructionFile[] | null>(null);
  let loadError = $state<string | null>(null);
  let editing = $state<string | null>(null);
  const root = $derived(settingsStore.root);

  async function load() {
    try {
      files = await listInstructionFiles(settingsStore.root);
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
    }
  }

  $effect(() => {
    void root;
    untrack(() => void load());
  });

  const slots = $derived(instructionSlots(settingsStore.info?.configDir ?? null, root, files ?? []));
  const others = $derived(otherInstructionFiles(files ?? [], slots));
  const lines = (content: string) => content.replace(/\n$/, "").split("\n").length;

  async function saved(label: string) {
    editing = null;
    pushToast(`${label} instructions saved`, "ok");
    await load();
  }
</script>

<div class="tab-body memory-tab">
  {#if loadError}
    <div class="settings-notice error"><Icon icon={AlertTriangle} size={13} /><span>Could not list instruction files: {loadError}</span></div>
  {/if}

  <SettingsGroup title="Instruction files" description="AGENTS.md files are part of every system prompt: user first, then the project, then yours.">
    <SettingsCard>
      {#each slots as slot (slot.path)}
        {#if editing === slot.path}
          <InstructionEditor {slot} onSaved={() => void saved(slot.label)} onCancel={() => (editing = null)} />
        {:else}
          <div class="extension-row">
            <div class="extension-row-copy">
              <div class="extension-row-title">
                <strong>{slot.label}</strong>
                <span class="extension-row-desc">{slot.description}</span>
              </div>
              <code class="extension-row-path" title={slot.path}>{slot.path}</code>
              <span class="extension-row-desc">
                {slot.file ? `${lines(slot.file.content)} lines` : "Not created yet, or empty"}
              </span>
            </div>
            <button type="button" class="btn-secondary size-s" disabled={editing !== null || files === null} onclick={() => (editing = slot.path)}>
              <Icon icon={slot.file ? Pencil : Plus} size={12} />
              <span>{slot.file ? "Edit" : "Create"}</span>
            </button>
          </div>
        {/if}
      {/each}
      {#if !root}
        <div class="extension-empty">Open a workspace to edit its AGENTS.md and AGENTS.local.md.</div>
      {/if}
    </SettingsCard>
  </SettingsGroup>

  {#if others.length > 0}
    <SettingsGroup title="Also loaded" description="Other instruction files found for this project. Edit them in your editor.">
      <SettingsCard>
        {#each others as file (file.path)}
          <div class="extension-row">
            <div class="extension-row-copy">
              <div class="extension-row-title">
                <Icon icon={FileText} size={12} />
                <code class="extension-row-path" title={file.path}>{file.path}</code>
                <Pill>{SCOPE_LABELS[file.scope]}</Pill>
              </div>
            </div>
          </div>
        {/each}
      </SettingsCard>
    </SettingsGroup>
  {/if}

  <p class="form-note">
    AGENTS.md files in subfolders are added when the agent works in those folders. Rules in <code>rules/</code> attach
    guidance to matching files (see Agents &amp; Commands).
    {#if settingsStore.settings?.compat.claude}CLAUDE.md files are read too while Claude compatibility is on (Advanced).{/if}
  </p>
</div>

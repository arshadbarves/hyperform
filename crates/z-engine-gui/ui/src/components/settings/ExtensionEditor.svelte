<script module lang="ts">
  import type { ExtensionKind } from "$lib/domain/settings/extensions";

  export type ExtensionLocation = "user" | "project";

  export interface ExtensionDraft {
    kind: ExtensionKind;
    /** A new file; otherwise `name` and `location` are fixed. */
    creating: boolean;
    name: string;
    location: ExtensionLocation;
    content: string;
    /** Shown above the editor, e.g. when the text was rebuilt. */
    notice: string | null;
    /** Saving would lose content the editor could not load. */
    blocked: boolean;
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import { writeExtensionFile } from "$lib/commands";
  import { EXTENSION_KINDS, extensionNameError, extensionTemplate } from "$lib/domain/settings/extensions";
  import { errorText } from "$lib/runtime";
  import { SegmentedChoice } from "$lib/ui";
  import { cancelOnEscape } from "./escape";

  type Props = {
    draft: ExtensionDraft;
    projectRoot: string | null;
    onSaved: (path: string) => void;
    onCancel: () => void;
  };

  let { draft, projectRoot, onSaved, onCancel }: Props = $props();

  const id = $props.id();
  const initial = untrack(() => draft);
  const meta = EXTENSION_KINDS.find((k) => k.kind === initial.kind) ?? EXTENSION_KINDS[0];
  let name = $state(initial.name);
  let location = $state<ExtensionLocation>(initial.location);
  let content = $state(initial.content || extensionTemplate(initial.kind, initial.name || "new"));
  let contentEdited = $state(Boolean(initial.content));
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    const next = name.trim() || "new";
    if (!untrack(() => contentEdited)) content = extensionTemplate(initial.kind, next);
  });

  const nameError = $derived(extensionNameError(initial.kind, name));
  const locations = $derived([
    { value: "user" as const, label: "User", description: "Available in every project." },
    ...(projectRoot ? [{ value: "project" as const, label: "This project", description: "Shared with the project in .hyperform/ (or .z-engine/)." }] : []),
  ]);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (nameError || initial.blocked) return;
    saving = true;
    try {
      const path = await writeExtensionFile(location === "project" ? projectRoot : null, initial.kind, name.trim(), content);
      onSaved(path);
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

</script>

<form class="settings-card extension-editor" onsubmit={save} {@attach cancelOnEscape(onCancel)}>
  <div class="extension-editor-head">
    <strong>{initial.creating ? `New ${meta.singular}` : `Edit ${meta.singular}`}</strong>
    {#if initial.notice}<span class="setting-note">{initial.notice}</span>{/if}
  </div>
  <div class="extension-editor-fields">
    <label class="settings-form-field" for={`${id}-name`}>
      <span>{meta.kind === "skills" ? "Folder name" : "File name"}</span>
      <input
        id={`${id}-name`}
        class="setting-input mono"
        bind:value={name}
        readonly={!initial.creating}
        placeholder={meta.kind === "commands" ? "review or frontend:component" : "name"}
        spellcheck={false}
      />
    </label>
    {#if initial.creating && locations.length > 1}
      <div class="settings-form-field">
        <span>Location</span>
        <SegmentedChoice label="Location" options={locations} value={location} onSelect={(next) => (location = next)} />
      </div>
    {/if}
  </div>
  <textarea
    class="setting-textarea mono extension-editor-text"
    rows="18"
    bind:value={content}
    spellcheck={false}
    aria-label="Markdown"
    oninput={() => (contentEdited = true)}
  ></textarea>
  {#if name.trim() && nameError}<p class="setting-error" role="alert">{nameError}</p>{/if}
  {#if error}<p class="setting-error" role="alert">{error}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || nameError !== null || initial.blocked}>
      {saving ? "Saving…" : "Save file"}
    </button>
  </div>
</form>

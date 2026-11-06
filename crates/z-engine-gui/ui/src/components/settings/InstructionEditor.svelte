<script lang="ts">
  import { untrack } from "svelte";
  import { writeInstructionFile } from "$lib/commands";
  import { isTruncated, type InstructionSlot } from "$lib/domain/settings/instructions";
  import { errorText } from "$lib/runtime";
  import { cancelOnEscape } from "./escape";

  type Props = { slot: InstructionSlot; onSaved: () => void; onCancel: () => void };
  let { slot, onSaved, onCancel }: Props = $props();

  const initial = untrack(() => slot);
  const truncated = isTruncated(initial.file?.content ?? "");
  let content = $state(initial.file?.content ?? "# Instructions\n\n");
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (truncated) return;
    saving = true;
    try {
      await writeInstructionFile(initial.path, content);
      onSaved();
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

</script>

<form class="settings-form memory-editor" onsubmit={save} {@attach cancelOnEscape(onCancel)}>
  <div class="extension-editor-head">
    <strong>{initial.label}</strong>
    <code class="extension-row-path">{initial.path}</code>
  </div>
  {#if truncated}
    <p class="setting-note">This file is larger than 64 KiB, so only its start was loaded. Edit it in your editor instead.</p>
  {/if}
  <textarea
    class="setting-textarea mono extension-editor-text"
    rows="16"
    bind:value={content}
    readonly={truncated}
    spellcheck={false}
    aria-label={`${initial.label} instructions`}
  ></textarea>
  {#if error}<p class="setting-error" role="alert">{error}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || truncated}>{saving ? "Saving…" : "Save file"}</button>
  </div>
</form>

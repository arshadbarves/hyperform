<script lang="ts">
  import { untrack } from "svelte";
  import { lspNameError, lspServerFrom, type LspForm } from "$lib/domain/settings/lspForm";
  import type { LspServerConfig } from "$lib/protocol/config/LspServerConfig";
  import { cancelOnEscape } from "./escape";

  type Props = {
    form: LspForm;
    taken: readonly string[];
    onSave: (name: string, server: LspServerConfig) => Promise<boolean>;
    onCancel: () => void;
  };

  let { form: initialForm, taken, onSave, onCancel }: Props = $props();

  const id = $props.id();
  const form = $state<LspForm>({ ...untrack(() => initialForm) });
  let saving = $state(false);
  let touched = $state(false);

  const built = $derived(lspServerFrom(form));
  const problem = $derived(lspNameError(form.name, taken) ?? ("error" in built ? built.error : null));

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

<form class="settings-form" onsubmit={submit} {@attach cancelOnEscape(onCancel)}>
  <div class="settings-form-row">
    <label class="settings-form-field" for={`${id}-name`}>
      <span>Name</span>
      <input id={`${id}-name`} class="setting-input mono" bind:value={form.name} placeholder="rust" spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-command`}>
      <span>Command</span>
      <input id={`${id}-command`} class="setting-input mono" bind:value={form.command} placeholder="rust-analyzer" spellcheck={false} />
    </label>
  </div>
  <label class="settings-form-field" for={`${id}-args`}>
    <span>Arguments</span>
    <input id={`${id}-args`} class="setting-input mono" bind:value={form.args} placeholder="--stdio" spellcheck={false} />
  </label>
  <div class="settings-form-row">
    <label class="settings-form-field" for={`${id}-ext`}>
      <span>File extensions</span>
      <input id={`${id}-ext`} class="setting-input mono" bind:value={form.extensions} placeholder="rs" spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-roots`}>
      <span>Root markers</span>
      <input id={`${id}-roots`} class="setting-input mono" bind:value={form.rootMarkers} placeholder="Cargo.toml" spellcheck={false} />
    </label>
  </div>
  <label class="settings-form-check">
    <input type="checkbox" bind:checked={form.enabled} />
    <span>Enabled</span>
  </label>
  {#if touched && problem}<p class="setting-error" role="alert">{problem}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || (touched && problem !== null)}>
      {saving ? "Saving…" : "Save server"}
    </button>
  </div>
</form>

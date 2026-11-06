<script lang="ts">
  import { untrack } from "svelte";
  import { CHECK_KINDS, checkError, DEFAULT_CHECK_TIMEOUT } from "$lib/domain/settings/checks";
  import { LIMITS, parseNumber } from "$lib/domain/settings/limits";
  import type { CheckKind } from "$lib/protocol/CheckKind";
  import type { CheckConfig } from "$lib/protocol/config/CheckConfig";
  import { SegmentedChoice } from "$lib/ui";
  import { cancelOnEscape } from "./escape";

  type Props = {
    check: CheckConfig;
    /** The file's other checks, for the unique-id rule. */
    siblings: readonly CheckConfig[];
    onSave: (check: CheckConfig) => Promise<boolean>;
    onCancel: () => void;
  };

  let { check, siblings, onSave, onCancel }: Props = $props();

  const KIND_OPTIONS = CHECK_KINDS.map((kind) => ({ value: kind, label: kind, description: kind }));
  const id = $props.id();
  const initial = untrack(() => check);
  let checkId = $state(initial.id);
  let label = $state(initial.label);
  let command = $state(initial.command);
  let kind = $state<CheckKind>(initial.kind);
  let cwd = $state(initial.cwd ?? "");
  let timeout = $state(String(initial.timeout_secs));
  let saving = $state(false);
  let touched = $state(false);

  const parsedTimeout = $derived(parseNumber(timeout, LIMITS.timeoutSecs));
  const candidate = $derived<CheckConfig>({
    id: checkId,
    label,
    command,
    kind,
    cwd: cwd.trim() || null,
    timeout_secs: parsedTimeout.ok ? (parsedTimeout.value ?? DEFAULT_CHECK_TIMEOUT) : DEFAULT_CHECK_TIMEOUT,
  });
  const problem = $derived(parsedTimeout.ok ? checkError(candidate, siblings) : `Timeout: ${parsedTimeout.error}`);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    touched = true;
    if (problem) return;
    saving = true;
    const saved = await onSave(candidate);
    saving = false;
    if (saved) onCancel();
  }
</script>

<form class="settings-form" onsubmit={submit} {@attach cancelOnEscape(onCancel)}>
  <div class="settings-form-row">
    <label class="settings-form-field" for={`${id}-id`}>
      <span>Id</span>
      <input id={`${id}-id`} class="setting-input mono" bind:value={checkId} placeholder="unit" spellcheck={false} />
    </label>
    <label class="settings-form-field" for={`${id}-label`}>
      <span>Label</span>
      <input id={`${id}-label`} class="setting-input" bind:value={label} placeholder="Blank shows the id" />
    </label>
  </div>
  <label class="settings-form-field" for={`${id}-command`}>
    <span>Command</span>
    <input id={`${id}-command`} class="setting-input mono" bind:value={command} placeholder="cargo test --workspace" spellcheck={false} />
  </label>
  <div class="settings-form-field">
    <span>Kind</span>
    <SegmentedChoice label="Check kind" options={KIND_OPTIONS} value={kind} onSelect={(next) => (kind = next)} />
  </div>
  <div class="settings-form-row">
    <label class="settings-form-field" for={`${id}-cwd`}>
      <span>Working directory</span>
      <input id={`${id}-cwd`} class="setting-input mono" bind:value={cwd} placeholder="Relative to the project root" spellcheck={false} />
    </label>
    <label class="settings-form-field narrow" for={`${id}-timeout`}>
      <span>Timeout (seconds)</span>
      <input id={`${id}-timeout`} class="setting-input" inputmode="numeric" bind:value={timeout} />
    </label>
  </div>
  {#if touched && problem}<p class="setting-error" role="alert">{problem}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || (touched && problem !== null)}>
      {saving ? "Saving…" : "Save check"}
    </button>
  </div>
</form>

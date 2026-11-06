<script lang="ts">
  import { untrack } from "svelte";
  import { DEFAULT_HOOK_TIMEOUT, hookError, type HookEventMeta } from "$lib/domain/settings/hooks";
  import { LIMITS, parseNumber } from "$lib/domain/settings/limits";
  import type { HookConfig } from "$lib/protocol/config/HookConfig";
  import { cancelOnEscape } from "./escape";

  type Props = {
    hook: HookConfig;
    event: HookEventMeta;
    /** Resolves to true once saved. */
    onSave: (hook: HookConfig) => Promise<boolean>;
    onCancel: () => void;
  };

  let { hook, event, onSave, onCancel }: Props = $props();

  const id = $props.id();
  const initial = untrack(() => hook);
  let matcher = $state(initial.matcher ?? "");
  let command = $state(initial.command);
  let timeout = $state(String(initial.timeout_secs));
  let saving = $state(false);
  let touched = $state(false);

  const parsedTimeout = $derived(parseNumber(timeout, LIMITS.timeoutSecs));
  const candidate = $derived<HookConfig>({
    matcher: matcher.trim() || null,
    command,
    timeout_secs: parsedTimeout.ok ? (parsedTimeout.value ?? DEFAULT_HOOK_TIMEOUT) : DEFAULT_HOOK_TIMEOUT,
  });
  const problem = $derived(parsedTimeout.ok ? hookError(candidate, event) : `Timeout: ${parsedTimeout.error}`);

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
  {#if event.matcher}
    <label class="settings-form-field" for={`${id}-matcher`}>
      <span>{event.matcher.label} (matcher)</span>
      <input id={`${id}-matcher`} class="setting-input mono" bind:value={matcher} placeholder={event.matcher.placeholder} spellcheck={false} />
    </label>
  {/if}
  <label class="settings-form-field" for={`${id}-command`}>
    <span>Command</span>
    <input id={`${id}-command`} class="setting-input mono" bind:value={command} placeholder="./scripts/entry-command.sh" spellcheck={false} />
  </label>
  <label class="settings-form-field narrow" for={`${id}-timeout`}>
    <span>Timeout (seconds)</span>
    <input id={`${id}-timeout`} class="setting-input" inputmode="numeric" bind:value={timeout} />
  </label>
  {#if touched && problem}<p class="setting-error" role="alert">{problem}</p>{/if}
  <div class="settings-form-actions">
    <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
    <button type="submit" class="btn-accent" disabled={saving || (touched && problem !== null)}>
      {saving ? "Saving…" : "Save hook"}
    </button>
  </div>
</form>

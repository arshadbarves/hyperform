<script lang="ts">
  import type { Snippet } from "svelte";
  import type { KeyPath } from "$lib/domain/settings/provenance";
  import { LAYER_LABELS, scopeLabel } from "$lib/domain/settings/scopes";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";
  import SourceBadge from "./SourceBadge.svelte";

  type Props = {
    title: string;
    description?: string;
    /** Enables the source badge (shown once a file sets the value), the override note and Reset. */
    keyPath?: KeyPath;
    controlId?: string;
    /** Title and control on one line, for switches. */
    inline?: boolean;
    error?: string | null;
    /** A short label beside the title, such as "Experimental". */
    tag?: string;
    children: Snippet;
  };

  let { title, description = "", keyPath, controlId, inline = false, error = null, tag, children }: Props = $props();

  let resetting = $state(false);
  let resetError = $state<string | null>(null);
  const source = $derived(keyPath ? settingsStore.sourceOf(keyPath) : null);
  const definedHere = $derived(keyPath ? settingsStore.scopeValue(keyPath) !== undefined : false);
  const shadow = $derived(keyPath ? settingsStore.shadowOf(keyPath) : null);
  const shown = $derived(error ?? resetError);

  async function reset() {
    if (!keyPath) return;
    resetting = true;
    resetError = await settingsStore.reset(keyPath);
    resetting = false;
  }
</script>

<div class="setting-row" class:inline data-setting={keyPath ? keyPath.join(".") : `@${title}`}>
  <div class="setting-row-head">
    <div class="setting-row-copy">
      {#if controlId}
        <label class="form-label-title" for={controlId}>{title}</label>
      {:else}
        <span class="form-label-title">{title}</span>
      {/if}
      {#if description}<span class="form-label-desc">{description}</span>{/if}
    </div>
    <div class="setting-row-meta">
      {#if tag}<Pill tone="attention">{tag}</Pill>{/if}
      {#if source && source !== "default"}<SourceBadge {source} />{/if}
      {#if definedHere}
        <button
          type="button"
          class="setting-reset"
          disabled={resetting}
          title={`Remove from ${scopeLabel(settingsStore.scope)} settings; lower layers apply again`}
          onclick={() => void reset()}
        >
          Reset
        </button>
      {/if}
      {#if inline}{@render children()}{/if}
    </div>
  </div>
  {#if !inline}{@render children()}{/if}
  {#if shadow}
    <p class="setting-note">
      {LAYER_LABELS[shadow]} settings override this, so edits to {scopeLabel(settingsStore.scope)} take effect only
      once that value is removed.
    </p>
  {/if}
  {#if shown}<p class="setting-error" role="alert">{shown}</p>{/if}
</div>

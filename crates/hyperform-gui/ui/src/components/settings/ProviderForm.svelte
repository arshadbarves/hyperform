<script lang="ts">
  import { untrack } from "svelte";
  import { openReleaseUrl } from "$lib/commands";
  import { credentialKey, keyHintText, keyStatus } from "$lib/domain/settings/credentials";
  import { formatKeyValues, parseKeyValues } from "$lib/domain/settings/formText";
  import { CACHE_OPTIONS, cacheChoice, cacheValue, PROVIDER_KIND_OPTIONS, type CacheChoice } from "$lib/domain/settings/options";
  import { connectFormError } from "$lib/domain/settings/providerWrites";
  import type { KeyStatus } from "$lib/protocol/config/KeyStatus";
  import type { ProviderKind } from "$lib/protocol/config/ProviderKind";
  import type { ProviderSettings } from "$lib/protocol/config/ProviderSettings";
  import { connectFormDefaults, requiresApiKey, type ConnectFormValues, type ProviderPreset } from "$lib/providers";
  import { Disclosure, SegmentedChoice } from "$lib/ui";
  import Icon, { ExternalLink, Eye, KeyRound, LoaderCircle } from "$lib/ui/icons";
  import CopilotSignIn from "./CopilotSignIn.svelte";

  /**
   * The fields that connect one provider: key (or sign-in), model, and
   * (folded away) the endpoint, wire format, headers and caching. Used by the
   * Settings dialog and by first-run setup.
   */
  type Props = {
    provider: ProviderPreset;
    model: string;
    live: ProviderSettings | null;
    credentials: Record<string, KeyStatus>;
    submitLabel?: string;
    scopeHint?: string;
    onCancel?: () => void;
    /** Resolves to the error to show, or null once connected. */
    onSave: (values: ConnectFormValues, apiKey: string) => Promise<string | null>;
  };

  let {
    provider,
    model: liveModel,
    live,
    credentials,
    submitLabel = "Connect",
    scopeHint,
    onCancel,
    onSave,
  }: Props = $props();

  const defaults = untrack(() => connectFormDefaults(provider, { model: liveModel, provider: live }));
  let apiKey = $state("");
  let model = $state(defaults.model);
  let baseUrl = $state(defaults.baseUrl);
  let kind = $state<ProviderKind>(defaults.kind);
  let headers = $state(formatKeyValues(defaults.headers, ":"));
  let cache = $state<CacheChoice>(cacheChoice(defaults.cacheControl));
  let showKey = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);

  const saved = $derived(keyStatus(credentials, credentialKey(baseUrl || provider.baseUrl)));
  const parsedHeaders = $derived(parseKeyValues(headers, ":"));
  const values = $derived<ConnectFormValues>({
    model: model.trim() || provider.defaultModel,
    baseUrl,
    kind,
    headers: parsedHeaders.values,
    cacheControl: cacheValue(cache),
  });
  const problem = $derived(parsedHeaders.error ?? connectFormError(provider, values, apiKey, saved.hasKey));
  const local = $derived(provider.tag === "Local");
  const signIn = $derived(provider.auth === "github-device");

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (problem) return;
    saving = true;
    error = await onSave(values, apiKey.trim());
    saving = false;
  }
</script>

<form class="provider-form" onsubmit={handleSubmit}>
  {#if signIn}
    <CopilotSignIn {baseUrl} {saved} onBaseUrl={(url) => (baseUrl = url)} />
  {:else if !local}
    <label class="provider-field-group">
      <div class="provider-field-label-row">
        <span class="provider-field-label">API key</span>
        {#if saved.hasKey}
          <span class="provider-saved-hint">{keyHintText(saved)}</span>
        {:else if provider.keyUrl}
          <button type="button" class="provider-key-link" onclick={() => provider.keyUrl && void openReleaseUrl(provider.keyUrl)}>
            <span>Get a key</span>
            <Icon icon={ExternalLink} size={11} />
          </button>
        {/if}
      </div>
      <div class="provider-key-input-wrap">
        <input
          type={showKey ? "text" : "password"}
          bind:value={apiKey}
          placeholder={saved.hasKey ? "Leave blank to keep the saved key" : provider.keyPlaceholder}
          spellcheck={false}
          autocomplete="off"
        />
        <button type="button" class="provider-eye-btn" title={showKey ? "Hide key" : "Show key"} onclick={() => (showKey = !showKey)}>
          <Icon icon={showKey ? KeyRound : Eye} size={13} />
        </button>
      </div>
      {#if !requiresApiKey(provider)}
        <small class="provider-field-hint">Free models work without a key; add one only for paid models.</small>
      {/if}
    </label>
  {/if}

  <label class="provider-field-group">
    <div class="provider-field-label-row">
      <span class="provider-field-label">Model</span>
      {#if provider.defaultModel && model !== provider.defaultModel}
        <button type="button" class="provider-model-reset" onclick={() => (model = provider.defaultModel)}>
          Use {provider.defaultModel}
        </button>
      {/if}
    </div>
    <input bind:value={model} placeholder={provider.defaultModel || "The model id your server serves"} spellcheck={false} />
  </label>

  <Disclosure open={provider.id === "custom"} class="provider-advanced" summaryClass="provider-advanced-toggle">
    {#snippet summary()}
      <span>Endpoint and advanced options</span>
      <span class="provider-advanced-url">{baseUrl || provider.baseUrl || "custom"}</span>
    {/snippet}
    <div class="provider-advanced-fields">
      {#if !signIn}
        <label class="provider-field-group">
          <span class="provider-field-label">API base URL</span>
          <input bind:value={baseUrl} placeholder={provider.baseUrl || "https://..."} spellcheck={false} />
        </label>
        <div class="provider-field-group">
          <span class="provider-field-label">Wire format</span>
          <SegmentedChoice label="Wire format" options={PROVIDER_KIND_OPTIONS} value={kind} onSelect={(next) => (kind = next)} />
        </div>
      {/if}
      <label class="provider-field-group">
        <span class="provider-field-label">Extra headers</span>
        <textarea class="setting-textarea mono" rows="3" bind:value={headers} placeholder="Name: value" spellcheck={false}></textarea>
        <small class="provider-field-hint">One per line. Headers merge across settings files.</small>
      </label>
      <div class="provider-field-group">
        <span class="provider-field-label">Prompt caching</span>
        <SegmentedChoice label="Prompt caching" options={CACHE_OPTIONS} value={cache} onSelect={(next) => (cache = next)} />
      </div>
    </div>
  </Disclosure>

  {#if error}
    <p class="setting-error" role="alert">{error}</p>
  {:else if problem}
    <p class="setting-note">{problem}</p>
  {/if}

  <div class="provider-form-foot">
    {#if scopeHint}<span class="provider-scope-hint">{scopeHint}</span>{/if}
    {#if onCancel}<button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>{/if}
    <button type="submit" class="btn-accent" disabled={saving || problem !== null}>
      {#if saving}
        <Icon icon={LoaderCircle} size={13} class="spin" />
        <span>Connecting…</span>
      {:else}
        <span>{submitLabel}</span>
      {/if}
    </button>
  </div>
</form>

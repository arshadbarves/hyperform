<script lang="ts">
  import { connectFormDefaults, detectProviderId, presetById, requiresApiKey, type ConnectFormValues } from "$lib/providers";
  import { connectProvider } from "$lib/stores/providerConnect";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Pill } from "$lib/ui";
  import Icon, { Cloud, Computer, KeyRound, LoaderCircle, Sparkles } from "$lib/ui/icons";
  import ProviderForm from "../settings/ProviderForm.svelte";
  import StepLayout from "./StepLayout.svelte";

  /** Which model answers: free with no key, your own key, or a model on this computer. */
  type Props = { onNext: () => void; onBack: () => void };
  let { onNext, onBack }: Props = $props();

  type Choice = "current" | "free" | "key" | "local";
  const KEY_PROVIDERS = ["anthropic", "openai", "openrouter", "github-copilot", "google", "deepseek", "groq", "mistral", "custom"];
  const LOCAL_PROVIDERS = ["ollama", "lmstudio"];

  const current = $derived(presetById(detectProviderId(settingsStore.settings?.provider.base_url)));
  const currentReady = $derived(
    Boolean(current && current.id !== "opencode" && (settingsStore.activeProvider?.hasKey || !requiresApiKey(current))),
  );
  let choice = $state<Choice>("free");
  let keyId = $state("anthropic");
  let localId = $state("ollama");
  let saving = $state(false);
  let error = $state<string | null>(null);

  const zen = presetById("opencode");
  const picked = $derived(presetById(choice === "key" ? keyId : localId));

  $effect(() => {
    if (currentReady) choice = "current";
  });

  async function save(values: ConnectFormValues, apiKey: string): Promise<string | null> {
    const preset = choice === "free" ? zen : picked;
    if (!preset) return "Pick a provider.";
    settingsStore.scope = "user";
    const failure = await connectProvider(preset, values, apiKey);
    if (!failure) onNext();
    return failure;
  }

  async function startFree() {
    if (!zen) return;
    saving = true;
    error = await save(connectFormDefaults(zen, { provider: null }), "");
    saving = false;
  }
</script>

<StepLayout title="Choose how HyperForm thinks" lead="You can change this any time in Settings › Providers.">
  <div class="choice-list" role="radiogroup" aria-label="Model provider">
    {#if currentReady && current}
      <button type="button" role="radio" aria-checked={choice === "current"} class="choice-card" class:is-selected={choice === "current"} onclick={() => (choice = "current")}>
        <span class="choice-icon"><Icon icon={KeyRound} size={16} /></span>
        <span class="choice-text">
          <span class="choice-title">Keep {current.name}</span>
          <span class="choice-desc">Already set up on this computer · {settingsStore.settings?.model.main}</span>
        </span>
      </button>
    {/if}
    <button type="button" role="radio" aria-checked={choice === "free"} class="choice-card" class:is-selected={choice === "free"} onclick={() => (choice = "free")}>
      <span class="choice-icon"><Icon icon={Sparkles} size={16} /></span>
      <span class="choice-text">
        <span class="choice-title">Start free <Pill tone="ok">Recommended to try</Pill></span>
        <span class="choice-desc">OpenCode Zen's free models. No account or key needed.</span>
      </span>
    </button>
    <button type="button" role="radio" aria-checked={choice === "key"} class="choice-card" class:is-selected={choice === "key"} onclick={() => (choice = "key")}>
      <span class="choice-icon"><Icon icon={Cloud} size={16} /></span>
      <span class="choice-text">
        <span class="choice-title">Use my API key</span>
        <span class="choice-desc">Anthropic, OpenAI, OpenRouter, Google and more, or sign in to GitHub Copilot.</span>
      </span>
    </button>
    <button type="button" role="radio" aria-checked={choice === "local"} class="choice-card" class:is-selected={choice === "local"} onclick={() => (choice = "local")}>
      <span class="choice-icon"><Icon icon={Computer} size={16} /></span>
      <span class="choice-text">
        <span class="choice-title">Run on this computer</span>
        <span class="choice-desc">Ollama or LM Studio. Nothing leaves your machine.</span>
      </span>
    </button>
  </div>

  {#if choice === "key" || choice === "local"}
    <div class="choice-detail">
      <div class="provider-chips" role="radiogroup" aria-label="Provider">
        {#each choice === "key" ? KEY_PROVIDERS : LOCAL_PROVIDERS as id (id)}
          {@const preset = presetById(id)}
          {#if preset}
            <button
              type="button"
              role="radio"
              aria-checked={(choice === "key" ? keyId : localId) === id}
              class="provider-chip"
              class:is-selected={(choice === "key" ? keyId : localId) === id}
              onclick={() => (choice === "key" ? (keyId = id) : (localId = id))}
            >
              {preset.name}
            </button>
          {/if}
        {/each}
      </div>
      {#if picked}
        {#key picked.id}
          <ProviderForm
            provider={picked}
            model=""
            live={null}
            credentials={settingsStore.credentials}
            submitLabel="Connect and continue"
            onSave={save}
          />
        {/key}
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn-ghost" onclick={onBack}>Back</button>
    {#if error}<p class="setting-error step-error" role="alert">{error}</p>{/if}
    {#if choice === "free"}
      <button type="button" class="btn-accent size-l" disabled={saving} onclick={() => void startFree()}>
        {#if saving}<Icon icon={LoaderCircle} size={13} class="spin" />{/if}
        <span>Continue</span>
      </button>
    {:else if choice === "current"}
      <button type="button" class="btn-accent size-l" onclick={onNext}>Continue</button>
    {/if}
  {/snippet}
</StepLayout>

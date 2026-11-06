<script lang="ts">
  import { saveApiKey } from "$lib/commands";
  import { credentialKey, keyHintText, keyStatus } from "$lib/domain/settings/credentials";
  import { PROVIDER_KIND_OPTIONS } from "$lib/domain/settings/options";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { detectProviderId, isProviderConnected, PROVIDERS, requiresApiKey, type ProviderPreset } from "$lib/providers";
  import { errorText, pushToast } from "$lib/runtime";
  import { connectProvider } from "$lib/stores/providerConnect";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, Pill } from "$lib/ui";
  import Icon, { KeyRound, Sparkles } from "$lib/ui/icons";
  import ProviderConnectModal from "./ProviderConnectModal.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SourceBadge from "./SourceBadge.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  let modalProvider = $state<ProviderPreset | null>(null);
  const activeId = $derived(detectProviderId(settings.provider.base_url));
  const activePreset = $derived(PROVIDERS.find((p) => p.id === activeId));

  /** The live endpoint for the active preset, else the preset's own; custom has none until active. */
  function endpointOf(p: ProviderPreset): string | null {
    return p.id === activeId ? settings.provider.base_url : p.baseUrl || null;
  }

  function bucketOf(p: ProviderPreset): string | null {
    const endpoint = endpointOf(p);
    return endpoint ? credentialKey(endpoint) : null;
  }

  const activeKey = $derived(keyStatus(settingsStore.credentials, credentialKey(settings.provider.base_url)));
  const kindLabel = $derived(PROVIDER_KIND_OPTIONS.find((o) => o.value === settings.provider.kind)?.label ?? settings.provider.kind);

  async function removeKey(p: ProviderPreset) {
    const baseUrl = endpointOf(p);
    if (!baseUrl) return;
    try {
      await saveApiKey(baseUrl, null);
      const removed = p.auth
        ? `Signed out of ${p.name}`
        : requiresApiKey(p)
          ? `${p.name} key removed`
          : `${p.name} key removed — free models still work`;
      pushToast(removed, "info");
    } catch (e) {
      pushToast(`Could not remove the key · ${errorText(e)}`, "warn");
    }
    await settingsStore.loadCredentials();
  }

</script>

<div class="tab-body providers-tab">
  <SettingsGroup title="Providers" description="Keys are stored once per API host in auth.json, never in settings files.">
    <SettingsCard>
      {#each PROVIDERS as p (p.id)}
        {@const isCurrent = p.id === activeId}
        {@const key = keyStatus(settingsStore.credentials, bucketOf(p))}
        {@const isConnected = isProviderConnected(p, isCurrent, key.hasKey)}
        <div class="provider-table-row">
          <div class="provider-row-left">
            <span class="provider-row-icon" style:color={p.color}>
              <Icon icon={Sparkles} size={15} />
            </span>
            <span class="provider-row-name">{p.name}</span>
            <Pill>{p.tag}</Pill>
            {#if isConnected}
              <Pill tone="ok" dot>Active</Pill>
            {:else if isCurrent}
              <Pill tone="attention">{p.auth ? "Active · needs sign-in" : "Active · needs a key"}</Pill>
            {/if}
            {#if key.hasKey}
              <span class="provider-key-badge" title="A key is stored for this API host">
                <Icon icon={KeyRound} size={10} /><span>{keyHintText(key)}</span>
              </span>
            {/if}
          </div>
          <div class="provider-row-right">
            {#if key.hasKey}
              <Button size="s" class="provider-remove-key" onclick={() => void removeKey(p)}>
                {p.auth ? "Sign out" : "Remove key"}
              </Button>
            {/if}
            <Button variant="secondary" size="s" onclick={() => (modalProvider = p)}>
              {isCurrent ? "Configure" : "Connect"}
            </Button>
          </div>
        </div>
      {/each}
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup title="In use" description="The endpoint and model new sessions start with.">
    <SettingsCard>
      <div class="active-model-summary">
        <div class="active-model-info">
          <span class="active-model-label">{activePreset?.name ?? "Custom endpoint"}</span>
          <code class="active-model-val">{settings.model.main}</code>
          <SourceBadge source={settingsStore.sourceOf(["provider", "base_url"])} />
        </div>
        <span class="active-model-hint">
          {settings.provider.base_url} · {kindLabel} ·
          {activeKey.hasKey
            ? keyHintText(activeKey)
            : activePreset && !requiresApiKey(activePreset)
              ? "no key needed"
              : activePreset?.auth
                ? "not signed in"
                : "no key stored"}
        </span>
      </div>
    </SettingsCard>
  </SettingsGroup>

  {#if modalProvider}
    {@const p = modalProvider}
    <ProviderConnectModal
      provider={p}
      model={settings.model.main}
      live={settings.provider}
      credentials={settingsStore.credentials}
      onClose={() => (modalProvider = null)}
      onSave={(values, apiKey) => connectProvider(p, values, apiKey)}
    />
  {/if}
</div>

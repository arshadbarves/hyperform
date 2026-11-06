<script lang="ts">
  import { saveSearchKey } from "$lib/commands";
  import { keyHintText, keyStatus, searchKeyBucket } from "$lib/domain/settings/credentials";
  import { SEARCH_BACKEND_OPTIONS } from "$lib/domain/settings/options";
  import type { WebSettings } from "$lib/protocol/config/WebSettings";
  import { errorText } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TextSetting from "./TextSetting.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  type Props = { web: WebSettings };
  let { web }: Props = $props();

  const id = $props.id();
  let key = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  const bucket = $derived(searchKeyBucket(web.search_backend));
  const stored = $derived(keyStatus(settingsStore.credentials, bucket));
  const backendLabel = $derived(SEARCH_BACKEND_OPTIONS.find((o) => o.value === web.search_backend)?.label ?? "");

  async function storeKey(value: string | null) {
    busy = true;
    try {
      await saveSearchKey(web.search_backend, value);
      error = null;
      key = "";
    } catch (e) {
      error = errorText(e);
    }
    await settingsStore.loadCredentials();
    busy = false;
  }

  const urlError = (text: string) => (text && !/^https?:\/\/\S+$/i.test(text) ? "Enter an http:// or https:// URL." : null);
</script>

<SettingsGroup title="Web" description="How the agent searches and fetches the web.">
  <SettingsCard>
    <ChoiceSetting title="Search backend" keyPath={["web", "search_backend"]} options={SEARCH_BACKEND_OPTIONS} value={web.search_backend} />
    {#if bucket}
      <SettingRow
        title={`${backendLabel} API key`}
        description={`Stored in auth.json for every project. ${bucket.toUpperCase()}_API_KEY in the environment wins over it.`}
        controlId={id}
        {error}
      >
        <form
          class="setting-add-row"
          onsubmit={(event) => {
            event.preventDefault();
            void storeKey(key.trim());
          }}
        >
          <input
            {id}
            class="setting-input mono"
            type="password"
            bind:value={key}
            placeholder={stored.hasKey ? keyHintText(stored) : "Paste the API key"}
            autocomplete="off"
            spellcheck={false}
          />
          {#if stored.hasKey}
            <button type="button" class="btn-ghost" disabled={busy} onclick={() => void storeKey(null)}>Remove</button>
          {/if}
          <button type="submit" class="btn-secondary" disabled={busy || !key.trim()}>Save key</button>
        </form>
      </SettingRow>
    {/if}
    {#if web.search_backend === "searxng" || web.search_url}
      <TextSetting
        title="SearXNG URL"
        description="Base URL of your SearXNG instance; required for the SearXNG backend."
        keyPath={["web", "search_url"]}
        value={web.search_url}
        optional
        mono
        placeholder="https://searx.example.org"
        validate={urlError}
      />
    {/if}
    <ToggleSetting
      title="Answer from fetched pages with the fast model"
      description="WebFetch returns the fast model's answer to the agent's question instead of the whole page. Off returns the page itself as markdown."
      keyPath={["web", "fetch_extract"]}
      value={web.fetch_extract}
    />
    <ToggleSetting
      title="Allow private network"
      description="Let WebFetch reach localhost and private addresses. Keep this off unless you need it."
      keyPath={["web", "allow_private_network"]}
      value={web.allow_private_network}
    />
  </SettingsCard>
</SettingsGroup>

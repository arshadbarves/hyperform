<script lang="ts">
  import { decisionsInUse, featureMode, featureModeOptions } from "$lib/domain/settings/features";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { featureStore } from "$lib/stores/features.svelte";
  import { EmptyState, Lightbulb } from "$lib/ui";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import DecisionModelCard from "./DecisionModelCard.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  /** Features still being measured, one Off / Shadow / On choice each, and the settings they share. */
  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const listed = $derived(featureStore.listed);
</script>

<div class="tab-body experimental-tab">
  <SettingsGroup
    title="Experimental features"
    description="Each one stays here until it meets a measured bar. Shadow runs a feature and records what it would have done in the Context tab without changing anything."
  >
    {#if !featureStore.loaded}
      <p class="settings-loading">Loading features…</p>
    {:else if listed.length === 0}
      <EmptyState
        icon={Lightbulb}
        title="No experimental features in this version"
        description="New features appear here first, off until you turn them on."
      />
    {:else}
      <SettingsCard>
        {#each listed as spec (spec.id)}
          <ChoiceSetting
            title={spec.title}
            description={spec.summary}
            keyPath={["experimental", spec.id]}
            options={featureModeOptions(spec)}
            value={featureMode(settings, spec.id)}
            tag="Experimental"
          />
        {/each}
      </SettingsCard>
    {/if}
  </SettingsGroup>

  {#if decisionsInUse(settings, featureStore.catalog)}
    <DecisionModelCard {settings} />
  {/if}
</div>

<script lang="ts">
  import { catalogContextWindow, catalogStore } from "$lib/catalog";
  import { LIMITS } from "$lib/domain/settings/limits";
  import { EFFORT_OPTIONS, type EffortChoice } from "$lib/domain/settings/options";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import ListSetting from "./ListSetting.svelte";
  import ModelField from "./ModelField.svelte";
  import NumberSetting from "./NumberSetting.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const catalog = bindStore(catalogStore);
  const model = $derived(settings.model);
  const catalogWindow = $derived(catalogContextWindow(catalog.current, model.main, settings.provider.base_url));

  $effect(() => {
    void catalogStore.ensure();
  });
</script>

<div class="tab-body">
  <SettingsGroup title="Models" description="Model ids as the active provider names them; the list shows its catalog.">
    <SettingsCard>
      <ModelField
        title="Main model"
        description="Runs the main agent and every custom agent that inherits its model."
        keyPath={["model", "main"]}
        value={model.main}
      />
      <ModelField
        title="Fast model"
        description="Titles, summaries, compaction and quick subagents. Blank uses the main model."
        keyPath={["model", "fast"]}
        value={model.fast}
        optional
        placeholder={`Same as main (${model.main})`}
      />
      <ModelField
        title="Review model"
        description="Review agents. Blank uses the main model."
        keyPath={["model", "review"]}
        value={model.review}
        optional
        placeholder={`Same as main (${model.main})`}
      />
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup collapsible title="Fallbacks" description="Tried in order when the main model fails with a retryable error.">
    <SettingsCard>
      <ListSetting
        title="Fallback models"
        description="A list set here replaces the lists of lower settings files."
        keyPath={["model", "fallbacks"]}
        items={model.fallbacks}
        placeholder="Model id, e.g. openai/gpt-4o"
      />
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup collapsible title="Requests" description="Limits sent with every model request.">
    <SettingsCard>
      <ChoiceSetting
        title="Reasoning effort"
        description="For models that think before answering. A session can still change it from the composer."
        keyPath={["model", "effort"]}
        options={EFFORT_OPTIONS}
        value={model.effort ?? "default"}
        toValue={(choice: EffortChoice) => (choice === "default" ? null : choice)}
      />
      <NumberSetting
        title="Max output tokens"
        description="Per-request output ceiling, from 256 to 200,000."
        keyPath={["model", "max_output_tokens"]}
        value={model.max_output_tokens}
        range={LIMITS.maxOutputTokens}
        unit="tokens"
      />
      <NumberSetting
        title="Context window"
        description="Overrides the catalog's window for the main model. Blank uses the catalog."
        keyPath={["model", "context_window"]}
        value={model.context_window}
        range={LIMITS.contextWindow}
        optional
        placeholder={catalogWindow ? `${catalogWindow.toLocaleString()} from the catalog` : "From the catalog"}
        unit="tokens"
      />
    </SettingsCard>
  </SettingsGroup>
</div>

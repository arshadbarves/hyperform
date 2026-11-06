<script lang="ts">
  import { testDecisionModel } from "$lib/commands";
  import { endpointProblem, testSummary, type TestTone } from "$lib/domain/settings/decisionModel";
  import { LIMITS } from "$lib/domain/settings/limits";
  import { RUNTIME_OPTIONS } from "$lib/domain/settings/nativeModel";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { errorText } from "$lib/runtime/toasts";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button } from "$lib/ui";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import DecisionNativeModel from "./DecisionNativeModel.svelte";
  import NumberSetting from "./NumberSetting.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TextSetting from "./TextSetting.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  /** `[decisions]`: the small model the decision features ask, shared by all of them. */
  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const decisions = $derived(settings.decisions);
  let testing = $state(false);
  let result = $state<{ tone: TestTone; text: string } | null>(null);

  async function test() {
    testing = true;
    result = null;
    try {
      result = testSummary(await testDecisionModel(settingsStore.root));
    } catch (e) {
      result = { tone: "error", text: errorText(e) };
    }
    testing = false;
  }
</script>

<SettingsGroup
  title="Decision model"
  description="A small local model answers the features' questions. Any slow, missing or unsure answer means the app does what it does today."
>
  <SettingsCard>
    <ChoiceSetting
      title="Runtime"
      keyPath={["decisions", "runtime"]}
      options={RUNTIME_OPTIONS}
      value={decisions.runtime}
    />
    <TextSetting
      title="Sidecar command"
      description="Starts laya-serve for you on a free local port with a new key each time; when set, the endpoint below is not used."
      keyPath={["decisions", "sidecar", "command"]}
      value={decisions.sidecar.command}
      placeholder="laya-serve"
      optional
      mono
    />
    <TextSetting
      title="Decision model endpoint"
      description="A laya-serve or Jev server already running. Only this machine, unless remote endpoints are allowed."
      keyPath={["decisions", "endpoint"]}
      value={decisions.endpoint}
      mono
      validate={endpointProblem}
    />
    <TextSetting
      title="API key variable"
      description="The environment variable holding the endpoint's key; leave blank when it needs none."
      keyPath={["decisions", "api_key_env"]}
      value={decisions.api_key_env}
      placeholder="LAYA_API_KEY"
      optional
      mono
    />
    <TextSetting
      title="Checkpoint"
      description="The model the sidecar or the app loads: multilingual, english or typed-decisions."
      keyPath={["decisions", "checkpoint"]}
      value={decisions.checkpoint}
      mono
    />
    <DecisionNativeModel checkpoint={decisions.checkpoint} />
    <NumberSetting
      title="Decision timeout"
      description="A slower answer counts as no answer."
      keyPath={["decisions", "timeout_ms"]}
      value={decisions.timeout_ms}
      range={LIMITS.decisionTimeoutMs}
      unit="ms"
    />
    <NumberSetting
      title="Confidence threshold"
      description="How sure an answer must be, 0 to 1, before a feature acts on it."
      keyPath={["decisions", "threshold"]}
      value={decisions.threshold}
      range={LIMITS.probability}
    />
    <NumberSetting
      title="Input length"
      description="Tokens the model reads per question; longer inputs are cut."
      keyPath={["decisions", "max_len"]}
      value={decisions.max_len}
      range={LIMITS.decisionMaxLen}
      unit="tokens"
    />
    <NumberSetting
      title="Questions per request"
      keyPath={["decisions", "max_batch"]}
      value={decisions.max_batch}
      range={LIMITS.decisionMaxBatch}
    />
    <ToggleSetting
      title="Allow a remote endpoint"
      description="Lets the endpoint be another machine; what the features ask about then leaves this one."
      keyPath={["decisions", "allow_remote"]}
      value={decisions.allow_remote}
    />
    <ToggleSetting
      title="Record decisions"
      description="Save each question's input and answer to a local dataset folder, for measuring and training."
      keyPath={["decisions", "record_dataset"]}
      value={decisions.record_dataset}
    />
    <SettingRow
      title="Test connection"
      description="Asks the model one known question, starting the sidecar or loading the in-app model first."
    >
      <div class="decision-test">
        <Button variant="secondary" size="s" disabled={testing} onclick={() => void test()}>
          {testing ? "Testing…" : "Test connection"}
        </Button>
        {#if result}<p class={`decision-test-result tone-${result.tone}`} role="status">{result.text}</p>{/if}
      </div>
    </SettingRow>
  </SettingsCard>
</SettingsGroup>

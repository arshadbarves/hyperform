<script lang="ts">
  import { CHECK_KINDS, unknownAutoChecks } from "$lib/domain/settings/checks";
  import { LIMITS } from "$lib/domain/settings/limits";
  import { VERIFICATION_MODE_OPTIONS } from "$lib/domain/settings/options";
  import type { Settings } from "$lib/protocol/config/Settings";
  import CheckListCard from "./CheckListCard.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import ListSetting from "./ListSetting.svelte";
  import NumberSetting from "./NumberSetting.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TrustNote from "./TrustNote.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const verification = $derived(settings.verification);
  const checkIds = $derived(verification.checks.map((check) => check.id));
  const suggestions = $derived([...CHECK_KINDS.filter((kind) => kind !== "custom"), ...checkIds]);
  const unknown = $derived(unknownAutoChecks(verification.auto_checks, checkIds));
</script>

<div class="tab-body verification-tab">
  <SettingsGroup title="Verification" description="What happens at the end of a turn that changed files.">
    <SettingsCard>
      <ChoiceSetting title="Mode" keyPath={["verification", "mode"]} options={VERIFICATION_MODE_OPTIONS} value={verification.mode} />
      <NumberSetting
        title="Max continuations"
        description="How often Auto and Strict send failures back to the agent in one turn, 0 to 10."
        keyPath={["verification", "max_continuations"]}
        value={verification.max_continuations}
        range={LIMITS.maxContinuations}
      />
      <ListSetting
        title="Auto checks"
        description="Check kinds or check ids run in Auto and Strict mode. A list set here replaces lower files' lists."
        keyPath={["verification", "auto_checks"]}
        items={verification.auto_checks}
        {suggestions}
        placeholder="test, lint, or a check id"
      />
    </SettingsCard>
    {#if unknown.length > 0}
      <p class="setting-note">
        {unknown.join(", ")} {unknown.length === 1 ? "matches" : "match"} no check kind or configured check, so
        nothing runs for {unknown.length === 1 ? "it" : "them"}.
      </p>
    {/if}
  </SettingsGroup>

  <TrustNote what="checks" />
  <CheckListCard />
</div>

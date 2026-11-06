<script lang="ts">
  import { PERMISSION_MODE_OPTIONS } from "$lib/domain/settings/options";
  import { stringList, unionItems } from "$lib/domain/settings/provenance";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import ListSetting from "./ListSetting.svelte";
  import RuleListCard from "./RuleListCard.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const DIRS = ["permissions", "additional_directories"];
  const dirs = $derived(stringList(settingsStore.scopeValue(DIRS)));
  const inheritedDirs = $derived.by(() => {
    const provenance = settingsStore.provenance;
    if (!provenance) return [];
    return unionItems(provenance, DIRS).filter((item) => !item.scopes.includes(settingsStore.scope));
  });
</script>

<div class="tab-body permissions-tab">
  <SettingsGroup title="Default mode" description="How new sessions start; the composer can switch a session's mode.">
    <SettingsCard>
      <ChoiceSetting
        title="Permission mode"
        keyPath={["permissions", "mode"]}
        options={PERMISSION_MODE_OPTIONS}
        value={settings.permissions.mode}
      />
      <ToggleSetting
        title="Start new chats in plan mode"
        description="The agent researches and proposes a plan, at any permission mode; nothing changes until you build it."
        keyPath={["permissions", "planning"]}
        value={settings.permissions.planning}
      />
      <ToggleSetting
        title="Run read-only shell commands without asking"
        description="Recognised read-only commands such as ls, cat or git status skip the approval prompt."
        keyPath={["permissions", "auto_allow_read_only_bash"]}
        value={settings.permissions.auto_allow_read_only_bash}
      />
    </SettingsCard>
  </SettingsGroup>

  <p class="form-note">
    Rules from every settings file combine. Deny wins in every mode, plan mode then refuses changes, bypass mode
    allows everything else, and ask wins over allow. Examples: <code>Bash(npm test:*)</code> <code>Edit(src/**)</code>
    <code>WebFetch(domain:docs.rs)</code> <code>mcp__github__create_issue</code>
  </p>

  <RuleListCard
    kind="allow"
    title="Allow"
    description="Run without asking."
    presets={["Bash(git status)", "Bash(git diff:*)", "Bash(npm test:*)", "Bash(cargo test:*)"]}
  />
  <RuleListCard
    kind="ask"
    title="Ask"
    description="Ask first, even when the mode or an allow rule would allow it."
    presets={["Bash(git push:*)"]}
  />
  <RuleListCard kind="deny" title="Deny" description="Never allowed, in any mode." presets={["Read(./.env)", "Read(~/.ssh/**)"]} />

  <SettingsGroup collapsible title="Folders" description="Where the agent may read and write outside the project.">
    <SettingsCard>
      <ListSetting
        title="Additional directories"
        description="Lists from every settings file combine."
        keyPath={DIRS}
        items={dirs}
        inherited={inheritedDirs}
        placeholder="/path/to/folder or ~/folder"
      />
    </SettingsCard>
  </SettingsGroup>
</div>

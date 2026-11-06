<script lang="ts">
  import { LIMITS } from "$lib/domain/settings/limits";
  import type { Settings } from "$lib/protocol/config/Settings";
  import LspCard from "./LspCard.svelte";
  import NumberSetting from "./NumberSetting.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import ShellCard from "./ShellCard.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";
  import WebCard from "./WebCard.svelte";
  import WorkspaceCard from "./WorkspaceCard.svelte";
  import { foldGroups } from "./folding";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  foldGroups();
</script>

<div class="tab-body advanced-tab">
  <SettingsGroup title="Context" description="How the prompt budget is kept in check during long sessions.">
    <SettingsCard>
      <NumberSetting
        title="Compact at"
        description="Summarize older history with the fast model when the context is this full, 50 to 99."
        keyPath={["context", "compact_at_percent"]}
        value={settings.context.compact_at_percent}
        range={LIMITS.compactAtPercent}
        unit="%"
      />
      <NumberSetting
        title="Recent tool results kept"
        description="Above half the window, older tool results are cleared; this many stay verbatim."
        keyPath={["context", "keep_recent_tool_results"]}
        value={settings.context.keep_recent_tool_results}
        range={LIMITS.keepRecentToolResults}
      />
      <ToggleSetting
        title="Repository map"
        description="Include an outline of the project's files and symbols in the system prompt."
        keyPath={["context", "repo_map"]}
        value={settings.context.repo_map}
      />
      <NumberSetting
        title="Repository map size"
        keyPath={["context", "repo_map_chars"]}
        value={settings.context.repo_map_chars}
        range={LIMITS.repoMapChars}
        unit="characters"
      />
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup title="Agent limits" description="Budgets for agent runs and subagents.">
    <SettingsCard>
      <NumberSetting
        title="Subagents at once"
        keyPath={["agents", "max_concurrent"]}
        value={settings.agents.max_concurrent}
        range={LIMITS.maxConcurrent}
      />
      <NumberSetting
        title="Subagent depth"
        description="How deep subagents may nest; 0 disables subagents."
        keyPath={["agents", "max_depth"]}
        value={settings.agents.max_depth}
        range={LIMITS.maxDepth}
      />
      <NumberSetting
        title="Turns per agent run"
        description="Model turns one agent may take before it stops."
        keyPath={["agents", "max_turns"]}
        value={settings.agents.max_turns}
        range={LIMITS.maxTurns}
      />
      <NumberSetting
        title="Session cost cap"
        description="Stop the session once it has spent this much; 0 turns the cap off."
        keyPath={["agents", "session_cost_cap_usd"]}
        value={settings.agents.session_cost_cap_usd}
        range={LIMITS.sessionCostCapUsd}
        unit="USD"
      />
    </SettingsCard>
  </SettingsGroup>

  <WebCard web={settings.web} />
  <ShellCard shell={settings.shell} />
  <LspCard lsp={settings.lsp} />
  <WorkspaceCard claude={settings.compat.claude} />
</div>

<script lang="ts">
  import { stringList, unionItems } from "$lib/domain/settings/provenance";
  import { sandboxPlatform, sandboxPlatformNote, writableDirError } from "$lib/domain/settings/sandbox";
  import { isMacPlatform, isWinPlatform } from "$lib/platform";
  import type { SandboxSettings } from "$lib/protocol/config/SandboxSettings";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import ListSetting from "./ListSetting.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  type Props = { sandbox: SandboxSettings };
  let { sandbox }: Props = $props();

  const EXTRA = ["shell", "sandbox", "extra_writable"];
  const note = sandboxPlatformNote(sandboxPlatform(isMacPlatform(), isWinPlatform()));
  const own = $derived(stringList(settingsStore.scopeValue(EXTRA)));
  const inherited = $derived.by(() => {
    const provenance = settingsStore.provenance;
    if (!provenance) return [];
    return unionItems(provenance, EXTRA).filter((item) => !item.scopes.includes(settingsStore.scope));
  });
</script>

<SettingsCard>
  <ToggleSetting
    title="Sandbox commands"
    description={`Bash commands, background shells and checks may only write inside the project, added directories, temp folders and tool caches. ${note}`}
    keyPath={["shell", "sandbox", "enabled"]}
    value={sandbox.enabled}
  />
  {#if sandbox.enabled}
    <ToggleSetting
      title="Run sandboxed commands without asking"
      description="Skips approval for commands whose writes stay inside the sandbox. Deny and ask rules and plan mode still apply."
      keyPath={["shell", "sandbox", "auto_allow"]}
      value={sandbox.auto_allow}
    />
    <ToggleSetting
      title="Allow network in the sandbox"
      description="Off blocks every connection except to localhost, including package downloads."
      keyPath={["shell", "sandbox", "allow_network"]}
      value={sandbox.allow_network}
    />
    <ListSetting
      title="Extra writable directories"
      description="More places sandboxed commands may write. ~/ is your home folder; relative paths start at the project. Lists from every file combine."
      keyPath={EXTRA}
      items={own}
      {inherited}
      placeholder="~/scratch"
      validate={writableDirError}
    />
  {/if}
</SettingsCard>

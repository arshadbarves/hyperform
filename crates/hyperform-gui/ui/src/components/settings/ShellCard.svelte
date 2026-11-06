<script lang="ts">
  import { stringList, unionItems } from "$lib/domain/settings/provenance";
  import type { ShellSettings } from "$lib/protocol/config/ShellSettings";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import KeyValueSetting from "./KeyValueSetting.svelte";
  import ListSetting from "./ListSetting.svelte";
  import SandboxCard from "./SandboxCard.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TextSetting from "./TextSetting.svelte";

  type Props = { shell: ShellSettings };
  let { shell }: Props = $props();

  const PASSTHROUGH = ["shell", "env_passthrough"];
  const own = $derived(stringList(settingsStore.scopeValue(PASSTHROUGH)));
  const inherited = $derived.by(() => {
    const provenance = settingsStore.provenance;
    if (!provenance) return [];
    return unionItems(provenance, PASSTHROUGH).filter((item) => !item.scopes.includes(settingsStore.scope));
  });
  const nameError = (name: string) => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) ? null : `${name} is not a variable name.`);
</script>

<SettingsGroup title="Shell" description="The shell that runs commands, the environment it sees, and the sandbox around it.">
  <SettingsCard>
    <TextSetting
      title="Shell"
      description="Executable used for commands. HYPERFORM_SHELL in the environment wins over it."
      keyPath={["shell", "path"]}
      value={shell.path}
      optional
      mono
      placeholder="Detected automatically"
    />
    <ListSetting
      title="Passed-through variables"
      description="Variables copied from the app's environment into commands. Lists from every file combine."
      keyPath={PASSTHROUGH}
      items={own}
      {inherited}
      placeholder="GITHUB_TOKEN"
      validate={nameError}
    />
    <KeyValueSetting
      title="Environment variables"
      description="Set for every command, one NAME=value per line."
      keyPath={["shell", "env"]}
      effective={shell.env}
    />
  </SettingsCard>
  <SandboxCard sandbox={shell.sandbox} />
</SettingsGroup>

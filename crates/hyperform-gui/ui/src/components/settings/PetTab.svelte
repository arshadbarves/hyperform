<script lang="ts">
  import { COMPANION_OPTIONS } from "$lib/domain/appearanceSettings";
  import { PET_LOOKS, PET_NAME_MAX, petName, type PetLook } from "$lib/domain/pet/looks";
  import { pose } from "$lib/domain/pet/pose";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { pet } from "$lib/runtime";
  import { petUi } from "$lib/stores/pet.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button } from "$lib/ui";
  import Pet from "../pet/Pet.svelte";
  import PetLookPicker from "../pet/PetLookPicker.svelte";
  import ChoiceSetting from "./ChoiceSetting.svelte";
  import SettingRow from "./SettingRow.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TextSetting from "./TextSetting.svelte";
  import ToggleSetting from "./ToggleSetting.svelte";

  /** Settings > Pet: a live preview, then its name and look, how lively it is, and whether it roams. */
  type Props = { settings: Settings };
  let { settings }: Props = $props();

  const PREVIEW = pose("happy", "center", "quiet", "sparkles");
  const look = $derived(settings.ui.pet.look);
  const lookLabel = $derived(PET_LOOKS.find((l) => l.id === look)?.label ?? "");
  let lookError = $state<string | null>(null);

  async function pickLook(next: PetLook) {
    if (next === look) return;
    lookError = await settingsStore.setValue(["ui", "pet", "look"], next);
  }

  function showCard() {
    ui.settingsOpen = false;
    petUi.openCard();
  }
</script>

<div class="tab-body pet-tab">
  <div class="pet-preview">
    <div class="pet-preview-stage" aria-hidden="true">
      <Pet pose={PREVIEW} {look} stage={pet.stage} wearing={pet.growth.wearing} size={112} />
    </div>
    <div class="pet-preview-copy">
      <p class="pet-preview-name">{petName(settings.ui.pet.name)}</p>
      <p class="pet-preview-sub">{lookLabel} · level {pet.level}</p>
      <Button size="s" variant="secondary" onclick={showCard}>Show its card</Button>
    </div>
  </div>

  <SettingsGroup title="Who it is" description="Its name shows on its card, in the command palette and when you meet it">
    <SettingsCard>
      <TextSetting
        title="Name"
        description={`Up to ${PET_NAME_MAX} characters.`}
        keyPath={["ui", "pet", "name"]}
        value={settings.ui.pet.name}
        placeholder="Zen"
        validate={(text) => (Array.from(text).length > PET_NAME_MAX ? `Keep it to ${PET_NAME_MAX} characters.` : null)}
      />
      <SettingRow title="Look" description="Tints its body. Its glow still shows the status: amber when something needs you." keyPath={["ui", "pet", "look"]} error={lookError}>
        <PetLookPicker value={look} onSelect={(next) => void pickLook(next)} label="Pet look" />
      </SettingRow>
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup title="How it behaves" description="The status line says the same at every level; the pet only echoes it">
    <SettingsCard>
      <ChoiceSetting
        title="Liveliness"
        description="Lively reacts to you too; calm reacts only to the agent and stays in the title bar."
        keyPath={["ui", "companion"]}
        options={COMPANION_OPTIONS}
        value={settings.ui.companion}
      />
      {#if settings.ui.companion === "lively"}
        <ToggleSetting
          title="Let it roam"
          description="While nothing needs it, it walks onto the composer, the sidebar and open panels. Drag it anywhere; double-click it for its card."
          keyPath={["ui", "pet", "roam"]}
          value={settings.ui.pet.roam}
        />
      {/if}
    </SettingsCard>
  </SettingsGroup>
</div>

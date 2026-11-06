<script lang="ts">
  import { PET_NAME_MAX, petName } from "$lib/domain/pet/looks";
  import { settingWrite } from "$lib/domain/settings/tomlValue";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { Button } from "$lib/ui";
  import Icon, { BadgeCheck, Folder, LoaderCircle, Shield } from "$lib/ui/icons";
  import PetLookPicker from "../pet/PetLookPicker.svelte";

  /** Meet the pet: it wakes above this card; you name it and pick its look, then start. */
  type Props = { onNext: () => void; onSkip: () => void };
  let { onNext, onSkip }: Props = $props();

  const PROMISES = [
    { icon: Folder, title: "Works inside your project", text: "It reads, edits and runs your code where it lives." },
    { icon: Shield, title: "Asks before it changes things", text: "You approve edits and commands, or let it go on its own." },
    { icon: BadgeCheck, title: "Checks its own work", text: "It runs your tests and tells you whether they pass." },
  ];

  let saving = $state(false);
  let error = $state<string | null>(null);
  const name = $derived(petName(onboarding.petName));

  async function keepPet(then: () => void) {
    saving = true;
    settingsStore.scope = "user";
    error = await settingsStore.apply([
      settingWrite(["ui", "pet", "name"], name),
      settingWrite(["ui", "pet", "look"], onboarding.petLook),
    ]);
    saving = false;
    if (!error) then();
  }
</script>

<div class="welcome">
  <h1 class="welcome-title">Hi, I'm {name}</h1>
  <p class="welcome-lead">
    I'll keep you company while HyperForm works inside your projects, and show you what it is doing at a glance.
  </p>

  <div class="meet-pet">
    <label class="meet-field">
      <span class="meet-label">Call me</span>
      <input
        class="setting-input"
        bind:value={onboarding.petName}
        maxlength={PET_NAME_MAX}
        placeholder="Zen"
        spellcheck="false"
        autocomplete="off"
        oninput={() => userSignals.typed()}
      />
    </label>
    <div class="meet-field">
      <span class="meet-label">Look</span>
      <PetLookPicker value={onboarding.petLook} onSelect={(look) => (onboarding.petLook = look)} />
    </div>
  </div>

  <ul class="welcome-promises">
    {#each PROMISES as item (item.title)}
      <li>
        <span class="welcome-icon" aria-hidden="true"><Icon icon={item.icon} size={16} strokeWidth={1.7} /></span>
        <span class="welcome-promise">
          <span class="welcome-promise-title">{item.title}</span>
          <span class="welcome-promise-text">{item.text}</span>
        </span>
      </li>
    {/each}
  </ul>
  {#if error}<p class="setting-error" role="alert">{error}</p>{/if}
  <div class="welcome-actions">
    <Button variant="accent" size="l" disabled={saving} onclick={() => void keepPet(onNext)}>
      {#if saving}<Icon icon={LoaderCircle} size={13} class="spin" />{/if}
      <span>Get started</span>
    </Button>
    <Button disabled={saving} onclick={() => void keepPet(onSkip)}>Skip setup</Button>
  </div>
</div>

<script lang="ts">
  import { PET_ACCESSORIES, PET_STAGES, PET_TRICKS, type PetAccessory } from "$lib/domain/pet/looks";
  import { pose } from "$lib/domain/pet/pose";
  import { settingWrite } from "$lib/domain/settings/tomlValue";
  import { pet } from "$lib/runtime";
  import { openSettings } from "$lib/stores/app-actions";
  import { petUi } from "$lib/stores/pet.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, ProgressRing } from "$lib/ui";
  import Icon, { Lock, Sliders } from "$lib/ui/icons";
  import Pet from "./Pet.svelte";

  /**
   * The pet's card, the one home for its name, level, XP, streak and what it
   * has unlocked. Customize leads to Settings > Pet, where its name and look
   * are chosen.
   */
  type Props = { onClose: () => void };
  let { onClose }: Props = $props();

  const HAPPY = pose("happy", "center", "quiet");
  const progress = $derived(pet.progress);
  const stageLabel = $derived(PET_STAGES.find((s) => s.id === pet.stage)?.label ?? "");
  const unlocked = $derived(new Set(pet.accessories));
  const tricks = $derived(PET_TRICKS.filter((t) => pet.level >= t.level).map((t) => t.label));
  const roaming = $derived(petUi.roam && settingsStore.settings?.ui.companion === "lively");

  function toggleWear(id: PetAccessory) {
    pet.wear(pet.growth.wearing === id ? null : id);
  }

  async function setRoam(roam: boolean) {
    settingsStore.scope = "user";
    await settingsStore.apply([settingWrite(["ui", "pet", "roam"], roam)]);
  }

  function customize() {
    onClose();
    openSettings("pet");
  }
</script>

<div class="pet-card-body">
  <header class="pet-card-head">
    <div class="pet-card-portrait">
      <ProgressRing value={progress.fraction} size={76} stroke={3} tone="ok" />
      <span class="pet-card-pet"><Pet pose={HAPPY} look={petUi.look} stage={pet.stage} wearing={pet.growth.wearing} size={56} /></span>
    </div>
    <div class="pet-card-who">
      <h2 class="pet-card-name">{petUi.name}</h2>
      <p class="pet-card-level">Level {progress.level} · {stageLabel}</p>
      <p class="pet-card-xp">{progress.into} of {progress.span} XP to level {progress.level + 1}</p>
      {#if pet.growth.streak >= 2}<p class="pet-card-streak">{pet.growth.streak} days in a row</p>{/if}
    </div>
  </header>

  <dl class="pet-card-stats">
    <div><dt>Turns</dt><dd>{pet.growth.turns}</dd></div>
    <div><dt>Verified</dt><dd>{pet.growth.verified}</dd></div>
    <div><dt>Applied</dt><dd>{pet.growth.applied}</dd></div>
  </dl>

  <section class="pet-card-section" aria-label="What it wears">
    <h3 class="pet-card-heading">Wears</h3>
    <div class="pet-card-wear">
      {#each PET_ACCESSORIES as item (item.id)}
        {@const open = unlocked.has(item.id)}
        <button
          type="button"
          class="pet-wear-chip"
          class:is-on={pet.growth.wearing === item.id}
          aria-pressed={pet.growth.wearing === item.id}
          disabled={!open}
          title={open ? item.label : `Unlocks at level ${item.level}`}
          onclick={() => toggleWear(item.id)}
        >
          {#if !open}<Icon icon={Lock} size={10} />{/if}
          {item.label}
          {#if !open}<span class="pet-wear-level">L{item.level}</span>{/if}
        </button>
      {/each}
    </div>
    {#if tricks.length}<p class="pet-card-note">Tricks: {tricks.join(", ")}</p>{/if}
  </section>

  <div class="pet-card-actions">
    <Button variant="secondary" size="s" onclick={customize}>
      <Icon icon={Sliders} size={12} />
      Customize…
    </Button>
    {#if settingsStore.settings?.ui.companion === "lively"}
      <Button size="s" onclick={() => void setRoam(!petUi.roam)}>{roaming ? "Stop roaming" : "Let it roam"}</Button>
    {/if}
    {#if !petUi.docked}
      <Button size="s" onclick={() => petUi.callBack()}>Call back</Button>
    {/if}
  </div>
</div>

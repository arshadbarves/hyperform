<script lang="ts">
  import { startStyle, styleSettings, type StartStyle } from "$lib/domain/modes";
  import type { PetPose } from "$lib/domain/pet/pose";
  import { settingWrite } from "$lib/domain/settings/tomlValue";
  import type { CompanionLevel } from "$lib/protocol/config/CompanionLevel";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Button, SegmentedChoice } from "$lib/ui";
  import Icon, { Eye, LoaderCircle, Pencil, Shield } from "$lib/ui/icons";
  import Pet from "../pet/Pet.svelte";
  import StepLayout from "./StepLayout.svelte";

  /** How much the agent may do on its own, how lively the pet is, and whether it roams. */
  type Props = { onNext: () => void; onBack: () => void };
  let { onNext, onBack }: Props = $props();

  const MODES: Array<{ id: StartStyle; title: string; desc: string; icon: typeof Shield }> = [
    { id: "default", title: "Ask before changes", desc: "It asks before editing files or running commands. The best place to start.", icon: Shield },
    { id: "acceptEdits", title: "Edit on its own", desc: "It edits files without asking, and still asks before running commands.", icon: Pencil },
    { id: "plan", title: "Plan first", desc: "It researches and proposes a plan. Nothing changes until you build it.", icon: Eye },
  ];
  const LEVELS = [
    { value: "lively" as const, label: "Lively", description: "Reacts to the agent and to you" },
    { value: "calm" as const, label: "Calm", description: "Reacts only to the agent, stays in the title bar" },
    { value: "off" as const, label: "Off", description: "A small dot instead" },
  ];
  const ROAM = [
    { value: "roam" as const, label: "Roams", description: "Walks onto the composer and panels while nothing needs it" },
    { value: "stay" as const, label: "Stays put", description: "Stays in the title bar and on Home" },
  ];
  const PREVIEW: Record<Exclude<CompanionLevel, "off">, PetPose> = {
    lively: { mood: "happy", gaze: "center", particles: "sparkles", tone: "ok" },
    calm: { mood: "idle", gaze: "center", particles: "none", tone: "quiet" },
  };

  const permissions = settingsStore.settings?.permissions;
  let mode = $state<StartStyle | null>(startStyle(permissions?.mode ?? "default", permissions?.planning ?? false));
  let level = $state<CompanionLevel>(settingsStore.settings?.ui.companion ?? "lively");
  let roam = $state<boolean>(settingsStore.settings?.ui.pet.roam ?? true);
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    saving = true;
    settingsStore.scope = "user";
    const style = mode ? [styleSettings(mode)] : [];
    error = await settingsStore.apply([
      ...style.flatMap((s) => [settingWrite(["permissions", "mode"], s.mode), settingWrite(["permissions", "planning"], s.planning)]),
      settingWrite(["ui", "companion"], level),
      settingWrite(["ui", "pet", "roam"], roam),
    ]);
    saving = false;
    if (!error) onNext();
  }
</script>

<StepLayout title="Decide how it works with you" lead="All of this can change any time: the mode from the composer, the rest in Settings.">
  <div class="choice-list" role="radiogroup" aria-label="Permission mode">
    {#each MODES as item (item.id)}
      <button type="button" role="radio" aria-checked={mode === item.id} class="choice-card" class:is-selected={mode === item.id} onclick={() => (mode = item.id)}>
        <span class="choice-icon"><Icon icon={item.icon} size={16} /></span>
        <span class="choice-text">
          <span class="choice-title">{item.title}</span>
          <span class="choice-desc">{item.desc}</span>
        </span>
      </button>
    {/each}
  </div>

  <div class="pet-pick">
    <div class="pet-pick-preview" aria-hidden="true">
      {#if level === "off"}
        <span class="pet-pick-dot"></span>
      {:else}
        <Pet pose={PREVIEW[level]} look={onboarding.petLook} size={48} />
      {/if}
    </div>
    <div class="pet-pick-choices">
      <SegmentedChoice label="How lively the pet is" options={LEVELS} value={level} onSelect={(next) => (level = next)} />
      {#if level === "lively"}
        <SegmentedChoice label="Whether the pet roams" options={ROAM} value={roam ? "roam" : "stay"} onSelect={(next) => (roam = next === "roam")} />
      {/if}
    </div>
  </div>

  {#snippet footer()}
    <Button onclick={onBack}>Back</Button>
    {#if error}<p class="setting-error step-error" role="alert">{error}</p>{/if}
    <Button variant="accent" size="l" disabled={saving} onclick={() => void save()}>
      {#if saving}<Icon icon={LoaderCircle} size={13} class="spin" />{/if}
      <span>Continue</span>
    </Button>
  {/snippet}
</StepLayout>

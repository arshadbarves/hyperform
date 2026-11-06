<script lang="ts">
  import { ONBOARDING_STEPS, onboardingPose, stepIndex } from "$lib/domain/onboarding";
  import { TYPING_MS } from "$lib/domain/pet/pose";
  import { composer } from "$lib/stores/composer.svelte";
  import { goHome } from "$lib/stores/app-actions";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { appear } from "$lib/ui/motion";
  import { ticker } from "$lib/ui/ticker.svelte";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import Pet from "../pet/Pet.svelte";
  import ModelStep from "./ModelStep.svelte";
  import ProjectStep from "./ProjectStep.svelte";
  import ReadyStep from "./ReadyStep.svelte";
  import StyleStep from "./StyleStep.svelte";
  import WelcomeStep from "./WelcomeStep.svelte";

  /**
   * First-run setup, full window: meet the pet, then model, project, work
   * style and ready. The pet stays above the card the whole way, big on the
   * first step, and reacts to each one.
   */
  type Props = { entering: boolean };
  let { entering }: Props = $props();

  const index = $derived(stepIndex(onboarding.step));
  const clock = ticker(() => userSignals.typingAt !== null, 400);
  const typing = $derived(userSignals.typingAt !== null && clock.now - userSignals.typingAt < TYPING_MS);
  const pose = $derived(onboardingPose(onboarding.step, typing));

  function finish(prompt: string | null) {
    onboarding.finish();
    goHome(onboarding.project);
    if (prompt) composer.setDraft(prompt);
  }
</script>

<div class="onboarding" class:is-entering={entering}>
  <header class="onboarding-bar" data-tauri-drag-region>
    {#if onboarding.step !== "welcome"}
      <div class="onboarding-progress" role="progressbar" aria-valuemin={1} aria-valuemax={ONBOARDING_STEPS.length} aria-valuenow={index + 1} aria-label="Setup progress">
        {#each ONBOARDING_STEPS as step, i (step)}
          <span class={`onboarding-dot${i < index ? " is-done" : ""}${i === index ? " is-current" : ""}`}></span>
        {/each}
      </div>
    {/if}
    <div class="onboarding-controls"><WindowControlsMaybe /></div>
  </header>

  <main class="onboarding-stage">
    <div class="onboarding-pet" class:is-hero={onboarding.step === "welcome"} data-splash-target>
      <Pet {pose} look={onboarding.petLook} size={132} />
    </div>
    {#key onboarding.step}
      <section class={`onboarding-card step-${onboarding.step}`} in:appear={{ y: 8 }}>
        {#if onboarding.step === "welcome"}
          <WelcomeStep onNext={() => onboarding.next()} onSkip={() => finish(null)} />
        {:else if onboarding.step === "model"}
          <ModelStep onNext={() => onboarding.next()} onBack={() => onboarding.back()} />
        {:else if onboarding.step === "project"}
          <ProjectStep onNext={() => onboarding.next()} onBack={() => onboarding.back()} />
        {:else if onboarding.step === "style"}
          <StyleStep onNext={() => onboarding.next()} onBack={() => onboarding.back()} />
        {:else}
          <ReadyStep onFinish={finish} onBack={() => onboarding.back()} />
        {/if}
      </section>
    {/key}
  </main>
</div>

import { nextStep, prevStep, type OnboardingStep } from "../domain/onboarding";
import { DEFAULT_PET_NAME, type PetLook } from "../domain/pet/looks";
import type { PetSettings } from "../protocol/config/PetSettings";

/**
 * First-run setup: whether it is showing, the step, the project added during
 * it, and the pet's name and look while you choose them (saved on Continue).
 */
class OnboardingStore {
  active = $state(false);
  step = $state<OnboardingStep>("welcome");
  project = $state<string | null>(null);
  petName = $state(DEFAULT_PET_NAME);
  petLook = $state<PetLook>("pearl");

  start(pet: PetSettings | null = null) {
    this.active = true;
    this.step = "welcome";
    this.project = null;
    this.petName = pet?.name ?? DEFAULT_PET_NAME;
    this.petLook = pet?.look ?? "pearl";
  }

  next() {
    this.step = nextStep(this.step);
  }

  back() {
    this.step = prevStep(this.step);
  }

  finish() {
    this.active = false;
  }
}

export const onboarding = new OnboardingStore();

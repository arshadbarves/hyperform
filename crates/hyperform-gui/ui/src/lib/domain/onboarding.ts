import { pose, type PetPose } from "./pet/pose";

/**
 * First run and the setup checklist. The welcome flow shows only on a fresh
 * install (no projects, no chats), so nothing new has to be stored: once a
 * project or a chat exists, the app opens normally.
 */
export function needsOnboarding(input: { projects: number; chats: number }): boolean {
  return input.projects === 0 && input.chats === 0;
}

export const ONBOARDING_STEPS = ["welcome", "model", "project", "style", "ready"] as const;
export type OnboardingStep = (typeof ONBOARDING_STEPS)[number];

/** The pet stays on screen through setup and reacts to each step. */
export function onboardingPose(step: OnboardingStep, typing: boolean): PetPose {
  if (typing) return pose("listening", "down", "quiet");
  switch (step) {
    case "welcome":
      return pose("greeting", "center", "quiet", "sparkles");
    case "model":
      return pose("thinking", "up", "quiet", "thought");
    case "project":
      return pose("reading", "scan", "quiet");
    case "style":
      return pose("curious", "up", "quiet");
    case "ready":
      return pose("happy", "center", "ok", "sparkles");
  }
}

export function stepIndex(step: OnboardingStep): number {
  return ONBOARDING_STEPS.indexOf(step);
}

export function nextStep(step: OnboardingStep): OnboardingStep {
  return ONBOARDING_STEPS[Math.min(ONBOARDING_STEPS.length - 1, stepIndex(step) + 1)];
}

export function prevStep(step: OnboardingStep): OnboardingStep {
  return ONBOARDING_STEPS[Math.max(0, stepIndex(step) - 1)];
}

export type SetupItemId = "model" | "trust" | "instructions";

export interface SetupItem {
  id: SetupItemId;
  label: string;
  hint: string;
  done: boolean;
}

export interface SetupChecklist {
  items: SetupItem[];
  done: number;
  total: number;
  complete: boolean;
}

/** What a project still needs; `null` means not known yet (or not applicable) and leaves the item out. */
export function setupChecklist(input: {
  modelReady: boolean;
  trusted: boolean | null;
  hasInstructions: boolean | null;
}): SetupChecklist {
  const items: SetupItem[] = [
    {
      id: "model",
      label: "Connect a model",
      hint: "Pick a provider, or start free with OpenCode Zen.",
      done: input.modelReady,
    },
  ];
  if (input.trusted !== null) {
    items.push({
      id: "trust",
      label: "Trust this project",
      hint: "Lets the project's own settings, hooks and tools run.",
      done: input.trusted,
    });
  }
  if (input.hasInstructions !== null) {
    items.push({
      id: "instructions",
      label: "Add an AGENTS.md",
      hint: "Tell the agent how this project builds, tests and is laid out.",
      done: input.hasInstructions,
    });
  }
  const done = items.filter((i) => i.done).length;
  return { items, done, total: items.length, complete: done === items.length };
}

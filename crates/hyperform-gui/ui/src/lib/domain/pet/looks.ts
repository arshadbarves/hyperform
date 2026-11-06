import type { PetLook } from "../../protocol/config/PetLook";

export type { PetLook };

/** The pet's name when none is set; the config crate uses the same default. */
export const DEFAULT_PET_NAME = "Zen";
/** Longest name, in characters (`ui.pet.name` is clamped to this). */
export const PET_NAME_MAX = 24;

export const PET_LOOKS: readonly { id: PetLook; label: string }[] = [
  { id: "pearl", label: "Pearl" },
  { id: "mint", label: "Mint" },
  { id: "sky", label: "Sky" },
  { id: "lilac", label: "Lilac" },
  { id: "peach", label: "Peach" },
  { id: "graphite", label: "Graphite" },
];

/** A name the pet can wear: trimmed, at most PET_NAME_MAX characters, never empty. */
export function petName(name: string | null | undefined): string {
  const trimmed = (name ?? "").trim();
  if (!trimmed) return DEFAULT_PET_NAME;
  return Array.from(trimmed).slice(0, PET_NAME_MAX).join("");
}

/** Growth changes the silhouette in four steps. */
export type PetStage = "seed" | "sprout" | "bloom" | "star";

export const PET_STAGES: readonly { id: PetStage; label: string; from: number }[] = [
  { id: "seed", label: "Seed", from: 1 },
  { id: "sprout", label: "Sprout", from: 3 },
  { id: "bloom", label: "Bloom", from: 6 },
  { id: "star", label: "Star", from: 10 },
];

export function petStage(level: number): PetStage {
  let stage: PetStage = "seed";
  for (const step of PET_STAGES) if (level >= step.from) stage = step.id;
  return stage;
}

/** Things to wear, each unlocked at a level; the pet card picks one. */
export type PetAccessory = "sprout" | "scarf" | "headphones" | "star";

export const PET_ACCESSORIES: readonly { id: PetAccessory; label: string; level: number }[] = [
  { id: "sprout", label: "Sprout", level: 2 },
  { id: "scarf", label: "Scarf", level: 4 },
  { id: "headphones", label: "Headphones", level: 6 },
  { id: "star", label: "Star", level: 9 },
];

/** Little moves the pet does while idle, each unlocked at a level. */
export type PetTrick = "stretch" | "hop" | "spin" | "sparkle";

export const PET_TRICKS: readonly { id: PetTrick; label: string; level: number }[] = [
  { id: "stretch", label: "Stretch", level: 1 },
  { id: "hop", label: "Hop", level: 3 },
  { id: "spin", label: "Spin", level: 5 },
  { id: "sparkle", label: "Sparkle", level: 7 },
];

export function unlockedAccessories(level: number): PetAccessory[] {
  return PET_ACCESSORIES.filter((a) => level >= a.level).map((a) => a.id);
}

export function unlockedTricks(level: number): PetTrick[] {
  return PET_TRICKS.filter((t) => level >= t.level).map((t) => t.id);
}

/** What reaching `level` newly unlocks (from a lower level `from`). */
export function unlocksBetween(from: number, level: number): { accessories: PetAccessory[]; tricks: PetTrick[] } {
  return {
    accessories: PET_ACCESSORIES.filter((a) => a.level > from && a.level <= level).map((a) => a.id),
    tricks: PET_TRICKS.filter((t) => t.level > from && t.level <= level).map((t) => t.id),
  };
}

export function isPetAccessory(value: unknown): value is PetAccessory {
  return PET_ACCESSORIES.some((a) => a.id === value);
}

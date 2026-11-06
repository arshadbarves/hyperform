import { boopReaction, type PetReaction } from "../domain/pet/behavior";
import { petName, type PetLook } from "../domain/pet/looks";
import type { PerchId } from "../domain/pet/perches";
import type { Point } from "../domain/pet/physics";
import type { PetRoutine } from "../domain/pet/routines";
import { settingsStore } from "./settings.svelte";

/** How long each reaction shows on the pet's face. */
const REACTION_MS: Record<PetReaction, number> = { booped: 900, giggle: 1200, levelUp: 1600, applied: 1500, dizzy: 1800 };

/**
 * The pet as the chrome sees it: who it is (from `[ui.pet]`), where it is
 * while it roams, what it is reacting to or playing, and which card is
 * open. UI-only; its growth lives in `lib/runtime/pet.svelte.ts`.
 */
class PetUi {
  /** The pet card, beside the pet wherever it is (the island while docked). */
  cardOpen = $state(false);
  /** Where the roaming pet is; "island" while it sits in the title bar. */
  perch = $state<PerchId>("island");
  /** The pet is in the island (not travelling to or from it). */
  docked = $state(true);
  /** The roaming pet's middle in viewport pixels, for the island's portrait to look at; null while docked. */
  at = $state<Point | null>(null);
  dragging = $state(false);
  /** Where an idle pet goes next (a wander, a drop, or "call back"). */
  wander = $state<PerchId | null>(null);
  /** The idle routine or trick playing now (`petIdle`). */
  routine = $state<PetRoutine | null>(null);
  #reaction = $state<{ kind: PetReaction; until: number } | null>(null);

  get name(): string {
    return petName(settingsStore.settings?.ui.pet.name);
  }
  get look(): PetLook {
    return settingsStore.settings?.ui.pet.look ?? "pearl";
  }
  get roam(): boolean {
    return settingsStore.settings?.ui.pet.roam ?? true;
  }

  /** The reaction still showing at `now`, if any. */
  reactionAt(now: number): PetReaction | null {
    const r = this.#reaction;
    return r && now < r.until ? r.kind : null;
  }

  react(kind: PetReaction, now = Date.now()) {
    this.#reaction = { kind, until: now + REACTION_MS[kind] };
  }

  /** A click on the pet: love, or a giggle when it comes quickly after the last one. */
  boop(now = Date.now()) {
    this.react(boopReaction(this.reactionAt(now)), now);
  }

  openCard() {
    this.cardOpen = true;
  }

  /** Back to the title bar until the next wander. */
  callBack() {
    this.wander = "island";
  }
}

export const petUi = new PetUi();

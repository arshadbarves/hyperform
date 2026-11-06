import { loadPetGrowth, savePetGrowth } from "../commands/pet";
import { applyGrowth, emptyGrowth, levelProgress, localDay, parseGrowth, wear, type PetGrowth } from "../domain/pet/growth";
import { petStage, unlockedAccessories, unlockedTricks, type PetAccessory, type PetTrick } from "../domain/pet/looks";
import type { PetGrowthSignal } from "../domain/sessions";

/** Changes settle this long before `pet.json` is written. */
const SAVE_DELAY_MS = 800;

/** A level the pet reached but has not celebrated yet (it waits until nothing needs you). */
export interface LevelUp {
  level: number;
  accessories: PetAccessory[];
  tricks: PetTrick[];
}

/**
 * The pet's growth, loaded from `<data dir>/pet.json` once and saved shortly
 * after each change. The rules are pure (`domain/pet/growth.ts`); signals
 * that arrive before the load are applied once it finishes.
 */
class PetRuntime {
  growth = $state<PetGrowth>(emptyGrowth());
  loaded = $state(false);
  levelUp = $state<LevelUp | null>(null);
  #early: { signal: PetGrowthSignal; at: Date }[] = [];
  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  get progress() {
    return levelProgress(this.growth.xp);
  }
  get level(): number {
    return this.progress.level;
  }
  get stage() {
    return petStage(this.level);
  }
  get accessories(): PetAccessory[] {
    return unlockedAccessories(this.level);
  }
  get tricks(): PetTrick[] {
    return unlockedTricks(this.level);
  }

  async load(): Promise<void> {
    try {
      this.growth = parseGrowth(await loadPetGrowth());
    } catch (e) {
      console.warn("the pet's growth could not be read; it starts fresh", e);
    }
    this.loaded = true;
    for (const { signal, at } of this.#early.splice(0)) this.record(signal, at);
  }

  record(signal: PetGrowthSignal, now = new Date()): void {
    if (!this.loaded) {
      this.#early.push({ signal, at: now });
      return;
    }
    const result = applyGrowth(this.growth, { ...signal, day: localDay(now) });
    if (result.state === this.growth) return;
    this.growth = result.state;
    if (result.levelTo > result.levelFrom) this.levelUp = { level: result.levelTo, ...result.unlocked };
    this.#saveSoon();
  }

  wear(accessory: PetAccessory | null): void {
    const next = wear(this.growth, accessory);
    if (next === this.growth) return;
    this.growth = next;
    this.#saveSoon();
  }

  celebrated(): void {
    this.levelUp = null;
  }

  #saveSoon(): void {
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => {
      savePetGrowth($state.snapshot(this.growth)).catch((e: unknown) => console.warn("the pet's growth was not saved", e));
    }, SAVE_DELAY_MS);
  }
}

export const pet = new PetRuntime();

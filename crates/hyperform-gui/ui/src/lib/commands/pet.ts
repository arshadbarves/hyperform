import { invoke } from "@tauri-apps/api/core";
import type { PetGrowth } from "../domain/pet/growth";

/** The saved growth as it was written (read it with `parseGrowth`), or null before the first save. */
export const loadPetGrowth = () => invoke<unknown>("pet_load");
export const savePetGrowth = (growth: PetGrowth) => invoke<void>("pet_save", { growth });

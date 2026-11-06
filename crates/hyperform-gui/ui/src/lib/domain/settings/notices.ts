import type { LoadedSettings } from "../../protocol/config/LoadedSettings";
import { LAYER_LABELS } from "./scopes";

export interface SettingsNotice {
  tone: "error" | "info" | "warn";
  text: string;
}

/** Skipped layers first, then v1-import notes, then the loader's other
 * warnings. Warnings that repeat a layer's error or note are dropped. */
export function settingsNotices(loaded: LoadedSettings | null): SettingsNotice[] {
  if (!loaded) return [];
  const errors: SettingsNotice[] = [];
  const notes: SettingsNotice[] = [];
  const repeats = new Set<string>();
  for (const layer of loaded.layers) {
    if (layer.error) {
      const where = layer.path ? ` (${layer.path})` : "";
      errors.push({ tone: "error", text: `${LAYER_LABELS[layer.scope]} settings${where} were skipped: ${layer.error}` });
      repeats.add(`skipped: ${layer.error}`);
    }
    if (layer.note) {
      notes.push({ tone: "info", text: layer.note });
      repeats.add(layer.note);
    }
  }
  const isRepeat = (warning: string) => [...repeats].some((text) => warning === text || warning.endsWith(text));
  const warnings = loaded.warnings.filter((warning) => !isRepeat(warning)).map((text) => ({ tone: "warn" as const, text }));
  return [...errors, ...notes, ...warnings];
}

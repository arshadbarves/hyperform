import { saveApiKey } from "../commands";
import { connectWrites, normalizeBaseUrl } from "../domain/settings/providerWrites";
import type { ConnectFormValues, ProviderPreset } from "../providers";
import { errorText, pushToast, sessions, setModel } from "../runtime";
import { settingsStore } from "./settings.svelte";

/**
 * Saves the key (when one was typed), makes the form's provider active in the
 * selected settings layer, and switches an open chat to the new model.
 * Returns the error to show, or null once connected.
 */
export async function connectProvider(
  preset: ProviderPreset,
  form: ConnectFormValues,
  apiKey: string,
): Promise<string | null> {
  if (apiKey) {
    try {
      await saveApiKey(normalizeBaseUrl(form.baseUrl), apiKey);
    } catch (e) {
      return `Could not save the key: ${errorText(e)}`;
    }
  }
  const error = await settingsStore.apply(connectWrites(form, settingsStore.settings?.provider ?? null));
  await settingsStore.loadCredentials();
  if (error) return error;
  if (sessions.activeId && form.model.trim()) void setModel(form.model.trim());
  pushToast(`Connected to ${preset.name}`, "ok");
  return null;
}

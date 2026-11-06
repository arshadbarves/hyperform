import type { ProviderSettings } from "../../protocol/config/ProviderSettings";
import { canSubmitProviderConnect, type ConnectFormValues, type ProviderPreset } from "../../providers";
import { sameJson, settingWrite, type SettingWrite } from "./tomlValue";

/** Stored without a trailing slash, like the config loader normalizes it. */
export function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}

/** Why the connect form cannot be saved yet, or null. */
export function connectFormError(
  preset: ProviderPreset,
  form: ConnectFormValues,
  apiKey: string,
  hasSavedKey: boolean,
): string | null {
  const url = normalizeBaseUrl(form.baseUrl);
  if (!url) return preset.auth ? "Enter your GitHub Enterprise domain." : "Enter the API base URL.";
  if (!/^https?:\/\/[^/\s]+/i.test(url)) return "The base URL must start with http:// or https://.";
  if (!form.model.trim()) return "Enter the model id to use.";
  if (!canSubmitProviderConnect(preset, apiKey, hasSavedKey)) {
    return preset.auth ? "Sign in with GitHub to connect." : `Enter your ${preset.name} API key.`;
  }
  return null;
}

/** Writes that make the form's provider active. Kind and URL are always
 * written, so a lower layer's endpoint cannot leak through; headers and
 * caching only when the form changed them. */
export function connectWrites(form: ConnectFormValues, current: ProviderSettings | null): SettingWrite[] {
  const writes = [
    settingWrite(["provider", "kind"], form.kind),
    settingWrite(["provider", "base_url"], normalizeBaseUrl(form.baseUrl)),
  ];
  const model = form.model.trim();
  if (model) writes.push(settingWrite(["model", "main"], model));
  if (!sameJson(form.headers, current?.headers ?? {})) {
    const headers = Object.keys(form.headers).length > 0 ? form.headers : null;
    writes.push(settingWrite(["provider", "headers"], headers));
  }
  if (form.cacheControl !== (current?.cache_control ?? null)) {
    writes.push(settingWrite(["provider", "cache_control"], form.cacheControl));
  }
  return writes;
}

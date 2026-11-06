import {
  appInfo,
  credentialStatus,
  getLayer,
  getSettings,
  removeSetting,
  setSetting,
  trustStatus,
  type AppInfo,
  type LayerFile,
  type LayerTarget,
  type TrustStatus,
} from "../commands";
import { credentialKey, keyStatus } from "../domain/settings/credentials";
import { rawAt, valueSource, type KeyPath, type LayerRaws, type Provenance } from "../domain/settings/provenance";
import { availableScopes, FILE_SCOPES, layerOf, outranks, type SettingsScope } from "../domain/settings/scopes";
import { settingWrite, type SettingWrite } from "../domain/settings/tomlValue";
import type { KeyStatus } from "../protocol/config/KeyStatus";
import type { LayerScope } from "../protocol/config/LayerScope";
import type { LoadedSettings } from "../protocol/config/LoadedSettings";
import type { Settings } from "../protocol/config/Settings";
import { errorText } from "../runtime/toasts";

/** Effective settings, the raw layer files, stored keys and trust for one
 * project root, plus the layer the Settings page writes to. */
class SettingsStore {
  info = $state.raw<AppInfo | null>(null);
  loaded = $state.raw<LoadedSettings | null>(null);
  files = $state.raw<Partial<Record<SettingsScope, LayerFile>>>({});
  credentials = $state.raw<Record<string, KeyStatus>>({});
  trust = $state.raw<TrustStatus | null>(null);
  root = $state.raw<string | null>(null);
  scope = $state<SettingsScope>("user");
  loadError = $state<string | null>(null);
  #generation = 0;

  get settings(): Settings | null {
    return this.loaded?.settings ?? null;
  }

  get scopes(): SettingsScope[] {
    return availableScopes(this.root);
  }

  get target(): LayerTarget {
    return { scope: this.scope, projectRoot: this.scope === "user" ? null : this.root };
  }

  get provenance(): Provenance | null {
    if (!this.loaded) return null;
    const raws: LayerRaws = {};
    for (const scope of FILE_SCOPES) raws[scope] = this.files[scope]?.raw;
    return { layers: this.loaded.layers, raws, effective: this.loaded.settings };
  }

  /** The active model API's endpoint and whether a key is stored for it. */
  get activeProvider(): { baseUrl: string; hasKey: boolean } | null {
    const provider = this.settings?.provider;
    if (!provider) return null;
    return { baseUrl: provider.base_url, hasKey: keyStatus(this.credentials, credentialKey(provider.base_url)).hasKey };
  }

  sourceOf(path: KeyPath): LayerScope {
    const provenance = this.provenance;
    return provenance ? valueSource(provenance, path) : "default";
  }

  /** The selected layer's own value at `path`, undefined when it sets none. */
  scopeValue(path: KeyPath, scope: SettingsScope = this.scope): unknown {
    return rawAt(this.files[scope]?.raw, path);
  }

  /** The higher layer that overrides whatever the selected layer sets at `path`. */
  shadowOf(path: KeyPath): LayerScope | null {
    const source = this.sourceOf(path);
    return outranks(source, layerOf(this.scope)) ? source : null;
  }

  async init(root: string | null) {
    await Promise.all([this.loadInfo(), this.load(root), this.loadCredentials()]);
  }

  /** Loads when the root changed or nothing is loaded yet. */
  async ensure(root: string | null) {
    if (root !== this.root || !this.loaded) await this.load(root);
  }

  async refresh() {
    await Promise.all([this.load(this.root), this.loadCredentials(), this.loadTrust()]);
  }

  /** Fresh data for the Settings page, for the project it was opened from. */
  async open(root: string | null) {
    await Promise.all([this.load(root), this.loadCredentials(), this.info ? null : this.loadInfo()]);
    await this.loadTrust();
  }

  async load(root: string | null) {
    const generation = ++this.#generation;
    const scopes = availableScopes(root);
    try {
      const [loaded, ...files] = await Promise.all([
        getSettings(root),
        ...scopes.map((scope) => getLayer(scope, scope === "user" ? null : root)),
      ]);
      if (generation !== this.#generation) return;
      this.loaded = loaded;
      this.files = Object.fromEntries(scopes.map((scope, i) => [scope, files[i]]));
      this.loadError = null;
      if (root !== this.root) {
        this.root = root;
        this.trust = null;
        if (!scopes.includes(this.scope)) this.scope = "user";
      }
    } catch (e) {
      if (generation === this.#generation) this.loadError = errorText(e);
    }
  }

  async loadInfo() {
    try {
      this.info = await appInfo();
    } catch (e) {
      console.warn("app_info unavailable", e);
    }
  }

  async loadCredentials() {
    try {
      this.credentials = await credentialStatus();
    } catch (e) {
      console.warn("credential_status unavailable", e);
    }
  }

  async loadTrust() {
    const root = this.root;
    if (!root) return;
    try {
      const trust = await trustStatus(root);
      if (root === this.root) this.trust = trust;
    } catch (e) {
      console.warn("trust_status unavailable", e);
    }
  }

  /** Runs a write against the selected layer, then refetches either way, since
   * a batch may have partly landed. Returns the error to show, or null. */
  async write(op: (target: LayerTarget) => Promise<unknown>): Promise<string | null> {
    let failure: string | null = null;
    try {
      await op(this.target);
    } catch (e) {
      failure = errorText(e);
    }
    await this.load(this.root);
    if (failure) return failure;
    return this.loadError ? `Saved, but the settings could not be reloaded: ${this.loadError}` : null;
  }

  /** Sets `path` in the selected layer; null removes it so lower layers apply. */
  setValue(path: KeyPath, value: unknown): Promise<string | null> {
    return this.apply([settingWrite(path, value)]);
  }

  /** Applies writes in order, stopping at the first failure, then refetches once. */
  apply(writes: readonly SettingWrite[]): Promise<string | null> {
    return this.write(async (target) => {
      for (const write of writes) {
        if (write.op === "set") await setSetting(target, write.keyPath, write.value);
        else await removeSetting(target, write.keyPath);
      }
    });
  }

  reset(path: KeyPath): Promise<string | null> {
    return this.write((target) => removeSetting(target, [...path]));
  }
}

export const settingsStore = new SettingsStore();

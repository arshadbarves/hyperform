import { featureCatalog } from "../commands";
import { listedFeatures } from "../domain/settings/features";
import type { FeatureSpec } from "../protocol/config/FeatureSpec";

/** The feature registry, loaded once: it only changes with the app version. */
class FeatureStore {
  catalog = $state.raw<FeatureSpec[]>([]);
  loaded = $state(false);
  #loading: Promise<void> | null = null;

  /** Experimental features built in this version. */
  get listed(): FeatureSpec[] {
    return listedFeatures(this.catalog);
  }

  ensure(): Promise<void> {
    this.#loading ??= featureCatalog()
      .then((catalog) => {
        this.catalog = catalog;
      })
      .catch((e: unknown) => {
        console.warn("feature_catalog unavailable", e);
        this.#loading = null;
      })
      .finally(() => {
        this.loaded = true;
      });
    return this.#loading;
  }
}

export const featureStore = new FeatureStore();

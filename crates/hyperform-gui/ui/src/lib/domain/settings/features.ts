import type { FeatureId } from "../../protocol/config/FeatureId";
import type { FeatureMode } from "../../protocol/config/FeatureMode";
import type { FeatureSpec } from "../../protocol/config/FeatureSpec";
import type { Settings } from "../../protocol/config/Settings";
import type { ChoiceOption } from "./options";
import type { SettingEntry } from "./searchIndex";

/**
 * The Experimental tab: which features it lists, the mode each one runs
 * in, and when the shared Decision model card shows.
 */

const MODE_OPTIONS: readonly ChoiceOption<FeatureMode>[] = [
  { value: "off", label: "Off", description: "The app behaves as if this feature did not exist." },
  {
    value: "shadow",
    label: "Shadow",
    description: "Runs and records what it would have done in the Context tab, but never changes a turn.",
  },
  {
    value: "on",
    label: "On",
    description: "Acts when the decision model is confident; any other answer keeps today's behavior.",
  },
];

/** Off, Shadow and On; Shadow only for features that support it. */
export function featureModeOptions(spec: FeatureSpec): ChoiceOption<FeatureMode>[] {
  return MODE_OPTIONS.filter((option) => option.value !== "shadow" || spec.supportsShadow);
}

/** Built in this version and still experimental, in registry order; stable features need no switch. */
export function listedFeatures(catalog: readonly FeatureSpec[]): FeatureSpec[] {
  return catalog.filter((spec) => spec.available && spec.stage === "experimental");
}

/** The mode in effect. Loading drops values that cannot apply, so a missing entry is off. */
export function featureMode(settings: Settings, id: FeatureId): FeatureMode {
  return settings.experimental[id] ?? "off";
}

/** The Decision model card shows while any listed decisions feature runs. */
export function decisionsInUse(settings: Settings, catalog: readonly FeatureSpec[]): boolean {
  return listedFeatures(catalog).some((spec) => spec.group === "decisions" && featureMode(settings, spec.id) !== "off");
}

const DECISION_MODEL_ENTRIES: readonly [string, string, string][] = [
  ["decisions.sidecar.command", "Sidecar command", "laya-serve start local decision model"],
  ["decisions.endpoint", "Decision model endpoint", "laya jev url server decision model"],
  ["decisions.timeout_ms", "Decision timeout", "latency wait decision model"],
  ["decisions.threshold", "Confidence threshold", "decision model confidence act"],
  ["decisions.record_dataset", "Record decisions", "dataset training decision model"],
];

/** Search entries for the listed features, and for the Decision model card while it shows. */
export function featureEntries(catalog: readonly FeatureSpec[], settings: Settings | null): SettingEntry[] {
  const features = listedFeatures(catalog).map((spec) => ({
    tab: "experimental" as const,
    key: `experimental.${spec.id}`,
    title: spec.title,
    group: null,
    words: `experimental shadow ${spec.summary.toLowerCase()}`,
  }));
  if (!settings || !decisionsInUse(settings, catalog)) return features;
  const model = DECISION_MODEL_ENTRIES.map(([key, title, words]) => ({
    tab: "experimental" as const,
    key,
    title,
    group: null,
    words,
  }));
  return [...features, ...model];
}

import type { SettingsScope } from "../../commands/settings";
import type { LayerScope } from "../../protocol/config/LayerScope";

export type { SettingsScope };

/** Writable layers, lowest precedence first. */
export const FILE_SCOPES: readonly SettingsScope[] = ["user", "project", "local"];

const PRECEDENCE: Record<LayerScope, number> = {
  default: 0,
  user: 1,
  project: 2,
  projectLocal: 3,
  env: 4,
};

export const LAYER_LABELS: Record<LayerScope, string> = {
  default: "Default",
  user: "User",
  project: "Project",
  projectLocal: "Personal",
  env: "Environment",
};

export const SCOPE_OPTIONS: ReadonlyArray<{ value: SettingsScope; label: string; description: string }> = [
  {
    value: "user",
    label: "User",
    description: "Your settings for every project, in settings.toml in the config folder.",
  },
  {
    value: "project",
    label: "This project",
    description: "Shared with everyone on the project, in .hyperform/settings.toml.",
  },
  {
    value: "local",
    label: "Personal (local)",
    description: "Only you, only here: .hyperform/settings.local.toml, kept out of git.",
  },
];

export function layerOf(scope: SettingsScope): LayerScope {
  return scope === "local" ? "projectLocal" : scope;
}

export function scopeOfLayer(layer: LayerScope): SettingsScope | null {
  if (layer === "user" || layer === "project") return layer;
  return layer === "projectLocal" ? "local" : null;
}

/** `a` wins over `b` when both define a value. */
export function outranks(a: LayerScope, b: LayerScope): boolean {
  return PRECEDENCE[a] > PRECEDENCE[b];
}

/** Project layers need an open project. */
export function availableScopes(projectRoot: string | null): SettingsScope[] {
  return projectRoot ? [...FILE_SCOPES] : ["user"];
}

export function scopeLabel(scope: SettingsScope): string {
  return LAYER_LABELS[layerOf(scope)];
}

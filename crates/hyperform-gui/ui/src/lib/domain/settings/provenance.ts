import type { LayerInfo } from "../../protocol/config/LayerInfo";
import type { LayerScope } from "../../protocol/config/LayerScope";
import { FILE_SCOPES, layerOf, type SettingsScope } from "./scopes";

export type KeyPath = readonly string[];

/** Each layer file's TOML table as JSON, from `get_layer`. */
export type LayerRaws = Partial<Record<SettingsScope, unknown>>;

export interface Provenance {
  layers: readonly LayerInfo[];
  raws: LayerRaws;
  /** Effective settings; they show when an environment variable overrides a file. */
  effective: unknown;
}

/** Keys the `HYPERFORM_*` variables override, with the value when no file sets them. */
const ENV_KEYS: ReadonlyArray<{ path: KeyPath; fallback: unknown }> = [
  { path: ["model", "main"], fallback: "anthropic/claude-sonnet-4.5" },
  { path: ["provider", "base_url"], fallback: "https://openrouter.ai/api/v1" },
  { path: ["provider", "kind"], fallback: "auto" },
  { path: ["shell", "path"], fallback: null },
];

const KIND_ALIASES: Record<string, string> = { openai: "openai_chat", "openai-chat": "openai_chat" };

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The value at `path`, or undefined when a layer does not set it. */
export function rawAt(raw: unknown, path: KeyPath): unknown {
  let current = raw;
  for (const key of path) {
    if (!isRecord(current) || !Object.hasOwn(current, key)) return undefined;
    current = current[key];
  }
  return current;
}

export function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function layerInfo(p: Provenance, scope: LayerScope): LayerInfo | undefined {
  return p.layers.find((layer) => layer.scope === scope);
}

/** The loader merged this layer: its file exists and was not skipped. */
export function layerActive(p: Provenance, scope: LayerScope): boolean {
  const info = layerInfo(p, scope);
  return Boolean(info?.exists && !info.error);
}

/** Why the loader skipped the layer, if it did. */
export function layerError(p: Provenance, scope: SettingsScope): string | null {
  return layerInfo(p, layerOf(scope))?.error ?? null;
}

function activeScopes(p: Provenance): SettingsScope[] {
  return FILE_SCOPES.filter((scope) => layerActive(p, layerOf(scope)));
}

function samePath(a: KeyPath, b: KeyPath): boolean {
  return a.length === b.length && a.every((key, i) => key === b[i]);
}

function canonical(path: KeyPath, value: unknown): unknown {
  if (typeof value !== "string") return value ?? null;
  const text = value.trim();
  if (samePath(path, ["provider", "base_url"])) return text.replace(/\/+$/, "");
  if (samePath(path, ["provider", "kind"])) return KIND_ALIASES[text] ?? text;
  return text;
}

function sameValue(path: KeyPath, a: unknown, b: unknown): boolean {
  return JSON.stringify(canonical(path, a)) === JSON.stringify(canonical(path, b));
}

/** The highest file layer that sets `path`. */
function fileWinner(p: Provenance, path: KeyPath): SettingsScope | null {
  const scopes = activeScopes(p);
  for (let i = scopes.length - 1; i >= 0; i--) {
    if (rawAt(p.raws[scopes[i]], path) !== undefined) return scopes[i];
  }
  return null;
}

/** The layer the effective value of `path` comes from. */
export function valueSource(p: Provenance, path: KeyPath): LayerScope {
  const winner = fileWinner(p, path);
  const env = ENV_KEYS.find((key) => samePath(key.path, path));
  if (env && layerActive(p, "env")) {
    const fileValue = winner ? rawAt(p.raws[winner], path) : env.fallback;
    if (!sameValue(path, rawAt(p.effective, path), fileValue)) return "env";
  }
  return winner ? layerOf(winner) : "default";
}

export interface UnionItem {
  value: string;
  scopes: SettingsScope[];
}

/** A list that unions across layers (rules, directories): each item with the layers listing it. */
export function unionItems(p: Provenance, path: KeyPath): UnionItem[] {
  const items: UnionItem[] = [];
  for (const scope of activeScopes(p)) {
    for (const value of stringList(rawAt(p.raws[scope], path))) {
      const item = items.find((existing) => existing.value === value);
      if (!item) items.push({ value, scopes: [scope] });
      else if (!item.scopes.includes(scope)) item.scopes.push(scope);
    }
  }
  return items;
}

/** The highest merged layer above `scope` that also defines entry `name` of a named table. */
export function namedOverride(
  p: Provenance,
  scope: SettingsScope,
  path: KeyPath,
  name: string,
): SettingsScope | null {
  return higherScopes(p, scope).find((s) => rawAt(p.raws[s], [...path, name]) !== undefined) ?? null;
}

/** The highest merged layer above `scope` with a list entry whose `id` is `id` (checks). */
export function idOverride(p: Provenance, scope: SettingsScope, path: KeyPath, id: string): SettingsScope | null {
  const hasId = (s: SettingsScope) => {
    const list = rawAt(p.raws[s], path);
    return Array.isArray(list) && list.some((entry) => isRecord(entry) && entry.id === id);
  };
  return higherScopes(p, scope).find(hasId) ?? null;
}

/** Merged layers above `scope`, highest first. */
function higherScopes(p: Provenance, scope: SettingsScope): SettingsScope[] {
  const above = FILE_SCOPES.slice(FILE_SCOPES.indexOf(scope) + 1);
  return above.filter((s) => layerActive(p, layerOf(s))).reverse();
}

/** Names of a layer's named table (`mcp.servers`, `lsp.servers`), sorted. */
export function tableNames(raw: unknown, path: KeyPath): string[] {
  const table = rawAt(raw, path);
  return isRecord(table) ? Object.keys(table).sort() : [];
}

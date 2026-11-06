import { isRecord, type KeyPath } from "./provenance";

/** What a settings file can hold: TOML has no null. */
export type TomlValue = string | number | boolean | TomlValue[] | { [key: string]: TomlValue };

export type SettingWrite =
  | { op: "set"; keyPath: string[]; value: TomlValue }
  | { op: "remove"; keyPath: string[] };

/** The value as `set_setting` sends it: null, undefined and non-finite numbers are dropped. */
export function toTomlValue(value: unknown): TomlValue | undefined {
  if (value === null || value === undefined) return undefined;
  if (typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") return Number.isFinite(value) ? value : undefined;
  if (Array.isArray(value)) {
    return value.map(toTomlValue).filter((item): item is TomlValue => item !== undefined);
  }
  if (!isRecord(value)) return undefined;
  const table: { [key: string]: TomlValue } = {};
  for (const [key, item] of Object.entries(value)) {
    const converted = toTomlValue(item);
    if (converted !== undefined) table[key] = converted;
  }
  return table;
}

/** An unset optional value (null) removes the key, so lower layers apply again. */
export function settingWrite(keyPath: KeyPath, value: unknown): SettingWrite {
  const toml = toTomlValue(value);
  return toml === undefined
    ? { op: "remove", keyPath: [...keyPath] }
    : { op: "set", keyPath: [...keyPath], value: toml };
}

/** Structural equality of two JSON-like values, ignoring object key order. */
export function sameJson(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((item, i) => sameJson(item, b[i]));
  }
  if (isRecord(a) && isRecord(b)) {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((key) => Object.hasOwn(b, key) && sameJson(a[key], b[key]));
  }
  return (a ?? null) === (b ?? null);
}

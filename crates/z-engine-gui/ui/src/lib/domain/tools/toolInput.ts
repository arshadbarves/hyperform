import type { JsonValue } from "../../protocol/serde_json/JsonValue";

/** Typed reads from untyped tool input JSON. */

export type JsonObject = { [key in string]: JsonValue };

export function asObject(value: JsonValue | undefined): JsonObject | null {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value : null;
}

/** First non-empty string among `keys`. */
export function str(input: JsonValue | undefined, ...keys: string[]): string {
  const obj = asObject(input);
  if (!obj) return "";
  for (const key of keys) {
    const value = obj[key];
    if (typeof value === "string" && value) return value;
  }
  return "";
}

export function num(input: JsonValue | undefined, key: string): number | null {
  const value = asObject(input)?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function bool(input: JsonValue | undefined, key: string): boolean {
  return asObject(input)?.[key] === true;
}

export function list(input: JsonValue | undefined, key: string): JsonValue[] {
  const value = asObject(input)?.[key];
  return Array.isArray(value) ? value : [];
}

/** Compact one-line JSON for generic rows; long payloads are clipped. */
export function compactJson(value: JsonValue | undefined, max = 120): string {
  if (value === undefined || value === null) return "";
  let raw: string;
  try {
    raw = JSON.stringify(value);
  } catch {
    return "";
  }
  if (raw === "{}" || raw === "[]") return "";
  return raw.length > max ? `${raw.slice(0, max - 1)}…` : raw;
}

export function prettyJson(value: JsonValue | undefined): string {
  if (value === undefined) return "";
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function firstLine(text: string, max = 160): string {
  const line = text.split("\n").find((l) => l.trim()) ?? "";
  const trimmed = line.trim();
  return trimmed.length > max ? `${trimmed.slice(0, max - 1)}…` : trimmed;
}

/** Show paths relative to the project root when they live inside it. */
export function relPath(path: string, root: string | null | undefined): string {
  if (!path || !root) return path;
  const base = root.replace(/[/\\]+$/, "");
  if (path === base) return ".";
  if (path.startsWith(`${base}/`) || path.startsWith(`${base}\\`)) return path.slice(base.length + 1);
  return path;
}

export function baseName(path: string): string {
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

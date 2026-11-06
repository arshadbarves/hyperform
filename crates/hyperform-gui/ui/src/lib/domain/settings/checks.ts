import type { CheckKind } from "../../protocol/CheckKind";
import type { CheckConfig } from "../../protocol/config/CheckConfig";
import { isRecord, rawAt } from "./provenance";

export const CHECK_KINDS: readonly CheckKind[] = ["test", "build", "lint", "typecheck", "format", "custom"];

export const DEFAULT_CHECK_TIMEOUT = 600;

export const CHECKS_PATH = ["verification", "checks"] as const;

/** Spellings the config crate accepts for each kind. */
const KIND_ALIASES: Record<string, CheckKind> = {
  test: "test",
  tests: "test",
  build: "build",
  compile: "build",
  lint: "lint",
  typecheck: "typecheck",
  types: "typecheck",
  check: "typecheck",
  format: "format",
  fmt: "format",
  custom: "custom",
};

export function parseCheckKind(value: unknown): CheckKind {
  return typeof value === "string" ? (KIND_ALIASES[value.trim().toLowerCase()] ?? "custom") : "custom";
}

export function emptyCheck(): CheckConfig {
  return { id: "", label: "", command: "", kind: "test", cwd: null, timeout_secs: DEFAULT_CHECK_TIMEOUT };
}

/** A layer file's check with the defaults the loader applies. */
export function normalizeCheck(raw: unknown): CheckConfig {
  const entry = isRecord(raw) ? raw : {};
  const text = (value: unknown) => (typeof value === "string" ? value : "");
  return {
    id: text(entry.id),
    label: text(entry.label),
    command: text(entry.command),
    kind: parseCheckKind(entry.kind),
    cwd: typeof entry.cwd === "string" && entry.cwd.trim() ? entry.cwd : null,
    timeout_secs: typeof entry.timeout_secs === "number" ? entry.timeout_secs : DEFAULT_CHECK_TIMEOUT,
  };
}

/** The checks one layer file defines. */
export function layerChecks(raw: unknown): CheckConfig[] {
  const list = rawAt(raw, CHECKS_PATH);
  return Array.isArray(list) ? list.map(normalizeCheck) : [];
}

/** Why the check cannot be saved, or null. `siblings` are the layer's other checks. */
export function checkError(check: CheckConfig, siblings: readonly CheckConfig[]): string | null {
  const id = check.id.trim();
  if (!id) return "Enter an id, such as unit or lint.";
  if (!/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(id)) return "Ids use letters, digits, _, - and .";
  if (siblings.some((other) => other.id.trim() === id)) return `This file already has a check named ${id}.`;
  if (!check.command.trim()) return "Enter the command to run.";
  if (!Number.isInteger(check.timeout_secs) || check.timeout_secs < 1) return "The timeout must be at least 1 second.";
  return null;
}

/** The check as written: trimmed, with an empty label left out so the id is shown. */
export function checkForFile(check: CheckConfig): Record<string, unknown> {
  const label = check.label.trim();
  return {
    id: check.id.trim(),
    ...(label ? { label } : {}),
    command: check.command.trim(),
    kind: check.kind,
    cwd: check.cwd?.trim() || null,
    timeout_secs: check.timeout_secs,
  };
}

/** Auto-check entries that name neither a check kind nor a configured check. */
export function unknownAutoChecks(entries: readonly string[], checkIds: readonly string[]): string[] {
  return entries.filter((entry) => !CHECK_KINDS.includes(entry as CheckKind) && !checkIds.includes(entry));
}

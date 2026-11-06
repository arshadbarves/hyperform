import type { InstructionFile } from "../../protocol/config/InstructionFile";
import type { SettingsScope } from "./scopes";

/** One editable instruction file: user, project or personal. */
export interface InstructionSlot {
  scope: SettingsScope;
  label: string;
  description: string;
  path: string;
  /** The discovered file, or null when it does not exist yet (or is blank). */
  file: InstructionFile | null;
}

/** Appended by the config crate to content cut at 64 KiB. */
const TRUNCATION_NOTE = "\n\n[Truncated: this file is larger than 64 KiB.]";

/** Saving truncated content would cut the file, so the editor refuses it. */
export function isTruncated(content: string): boolean {
  return content.endsWith(TRUNCATION_NOTE);
}

/** Joins with the separator the folder already uses, so Windows paths stay Windows paths. */
export function joinPath(dir: string, name: string): string {
  const windows = dir.includes("\\") && !dir.includes("/");
  const sep = windows ? "\\" : "/";
  return `${dir.replace(/[\\/]+$/, "")}${sep}${name}`;
}

/** Same file after normalizing separators and a trailing slash; case-insensitive,
 * as on the default macOS and Windows filesystems. */
export function samePath(a: string, b: string): boolean {
  const norm = (path: string) => path.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return norm(a) === norm(b);
}

export function instructionSlots(
  configDir: string | null,
  projectRoot: string | null,
  files: readonly InstructionFile[],
): InstructionSlot[] {
  const find = (path: string) => files.find((file) => samePath(file.path, path)) ?? null;
  const slots: InstructionSlot[] = [];
  const add = (scope: SettingsScope, label: string, description: string, path: string) =>
    slots.push({ scope, label, description, path, file: find(path) });
  if (configDir) {
    add("user", "User", "Your instructions for every project.", joinPath(configDir, "AGENTS.md"));
  }
  if (projectRoot) {
    add("project", "This project", "Shared with the team; commit it with the code.", joinPath(projectRoot, "AGENTS.md"));
    add("local", "Personal (local)", "Only for you in this project; keep it out of git.", joinPath(projectRoot, "AGENTS.local.md"));
  }
  return slots;
}

/** Discovered files the slots do not cover: CLAUDE.md, parent folders, nested folders. */
export function otherInstructionFiles(files: readonly InstructionFile[], slots: readonly InstructionSlot[]): InstructionFile[] {
  return files.filter((file) => !slots.some((slot) => samePath(slot.path, file.path)));
}

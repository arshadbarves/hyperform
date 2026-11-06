import type { Effort } from "../protocol/Effort";
import type { PermissionMode } from "../protocol/PermissionMode";

export interface ModeMeta {
  id: PermissionMode;
  label: string;
  description: string;
  /** Shown before switching into a mode that removes guardrails. */
  warning?: string;
}

/** Permission levels: how often the agent asks. Plan mode is a separate switch. */
export const MODES: ModeMeta[] = [
  { id: "default", label: "Ask", description: "Ask before gated edits and commands" },
  { id: "acceptEdits", label: "Auto-accept edits", description: "Apply edits in allowed folders without asking" },
  {
    id: "bypass",
    label: "Bypass",
    description: "Approve everything except explicit deny rules",
    warning: "Edits and commands run without asking. Use it only in a sandbox or a disposable checkout.",
  },
];

export function modeMeta(mode: PermissionMode): ModeMeta {
  return MODES.find((m) => m.id === mode) ?? MODES[0];
}

export interface PlanMeta {
  label: string;
  description: string;
  on: string;
  off: string;
}

/** The plan switch, shown as its own chip next to the permission level. */
export const PLAN: PlanMeta = {
  label: "Plan",
  description: "Research and propose a plan; nothing changes until you build it",
  on: "Plan first",
  off: "Stop planning",
};

/** Shift+Tab toggles plan mode; the permission level stays as it is. */
export function togglePlanning(planning: boolean): boolean {
  return !planning;
}

/** The composer's mode chips: the removable Plan chip only while planning, and the level. */
export function modeChips(planning: boolean, mode: PermissionMode): { plan: PlanMeta | null; level: ModeMeta } {
  return { plan: planning ? PLAN : null, level: modeMeta(mode) };
}

/** Onboarding's starting styles; "plan" is plan mode at the Ask level. */
export type StartStyle = "default" | "acceptEdits" | "plan";

export function styleSettings(style: StartStyle): { mode: PermissionMode; planning: boolean } {
  return style === "plan" ? { mode: "default", planning: true } : { mode: style, planning: false };
}

/** The style the current settings match; bypass matches none. */
export function startStyle(mode: PermissionMode, planning: boolean): StartStyle | null {
  if (planning) return "plan";
  return mode === "bypass" ? null : mode;
}

export interface EffortMeta {
  id: Effort;
  description: string;
}

export const EFFORTS: EffortMeta[] = [
  { id: "low", description: "Fast and cheap" },
  { id: "medium", description: "Balanced default" },
  { id: "high", description: "Thorough thinking" },
  { id: "max", description: "Maximum depth" },
];

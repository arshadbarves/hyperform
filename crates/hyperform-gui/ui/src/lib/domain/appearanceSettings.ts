import type { CompanionLevel } from "../protocol/config/CompanionLevel";
import type { TaskReportView } from "../protocol/config/TaskReportView";
import type { ChoiceOption } from "./settings/options";

export const COMPANION_OPTIONS: readonly ChoiceOption<CompanionLevel>[] = [
  { value: "lively", label: "Lively", description: "Reacts to the agent and to what you do: typing, scrolling, stepping away." },
  { value: "calm", label: "Calm", description: "Reacts only to the agent's work, and stays in the title bar." },
  { value: "off", label: "Off", description: "No pet; the status line keeps a small dot." },
];

export interface AppearanceOption {
  value: TaskReportView;
  label: string;
  description: string;
}

export const APPEARANCE_OPTIONS: readonly AppearanceOption[] = [
  {
    value: "quiet",
    label: "Quiet",
    description: "Shows the final result, with verification tucked away until you open it.",
  },
  {
    value: "compact",
    label: "Compact",
    description: "Adds a short outcome and check count while keeping supporting evidence collapsed.",
  },
  {
    value: "detailed",
    label: "Detailed",
    description: "Keeps verification checks and supporting evidence visible for every completed task.",
  },
];

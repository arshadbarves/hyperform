import type { Effort } from "../../protocol/Effort";
import type { PermissionMode } from "../../protocol/PermissionMode";
import type { ProviderKind } from "../../protocol/config/ProviderKind";
import type { SearchBackend } from "../../protocol/config/SearchBackend";
import type { VerificationMode } from "../../protocol/VerificationMode";
import { EFFORTS, MODES } from "../modes";

/** An option of a segmented settings choice. */
export interface ChoiceOption<T extends string> {
  value: T;
  label: string;
  description: string;
}

/** `default` stands for an unset effort: the model decides. */
export type EffortChoice = Effort | "default";

export const EFFORT_OPTIONS: readonly ChoiceOption<EffortChoice>[] = [
  { value: "default", label: "Default", description: "No effort is sent; the model uses its own default." },
  ...EFFORTS.map((effort) => ({ value: effort.id, label: effort.id, description: effort.description })),
];

export const PERMISSION_MODE_OPTIONS: readonly ChoiceOption<PermissionMode>[] = MODES.map((mode) => ({
  value: mode.id,
  label: mode.label,
  description: mode.warning ? `${mode.description}. ${mode.warning}` : `${mode.description}.`,
}));

export const VERIFICATION_MODE_OPTIONS: readonly ChoiceOption<VerificationMode>[] = [
  { value: "off", label: "Off", description: "Nothing runs and turns carry no verification badge." },
  {
    value: "report",
    label: "Report",
    description: "Turns get a Verified, Unverified or Failed badge from the checks that ran; nothing extra runs.",
  },
  {
    value: "auto",
    label: "Auto",
    description: "When a turn changed files, the auto checks run at its end and failures go back to the agent, up to the continuation limit.",
  },
  {
    value: "strict",
    label: "Strict",
    description: "Like Auto, and the agent keeps working until the checks pass or the continuation limit is reached.",
  },
];

export const PROVIDER_KIND_OPTIONS: readonly ChoiceOption<ProviderKind>[] = [
  { value: "auto", label: "Auto", description: "Pick the wire format from the base URL." },
  { value: "openai_chat", label: "OpenAI-compatible", description: "Chat completions: OpenRouter, OpenCode Zen, local servers." },
  { value: "anthropic", label: "Anthropic", description: "The Anthropic Messages API." },
];

/** `auto` stands for an unset `cache_control`: on where the provider supports it. */
export type CacheChoice = "auto" | "on" | "off";

export const CACHE_OPTIONS: readonly ChoiceOption<CacheChoice>[] = [
  { value: "auto", label: "Auto", description: "Prompt caching where the provider supports it." },
  { value: "on", label: "On", description: "Always mark cache breakpoints." },
  { value: "off", label: "Off", description: "Never ask the provider to cache prompts." },
];

export function cacheChoice(value: boolean | null): CacheChoice {
  return value === null ? "auto" : value ? "on" : "off";
}

export function cacheValue(choice: CacheChoice): boolean | null {
  return choice === "auto" ? null : choice === "on";
}

export const SEARCH_BACKEND_OPTIONS: readonly ChoiceOption<SearchBackend>[] = [
  { value: "none", label: "Off", description: "Web search is unavailable to the agent." },
  { value: "brave", label: "Brave", description: "Brave Search API; needs a key." },
  { value: "tavily", label: "Tavily", description: "Tavily search API; needs a key." },
  { value: "exa", label: "Exa", description: "Exa search API; needs a key." },
  { value: "searxng", label: "SearXNG", description: "Your own SearXNG instance at the search URL; no key." },
];

import type { Usage } from "../protocol/Usage";
import { fmtTokens } from "../util";

/** Every prompt token the model saw, cached or not. */
export function promptTokens(u: Usage): number {
  return u.inputTokens + u.cacheReadTokens + u.cacheWriteTokens;
}

export function totalTokens(u: Usage): number {
  return promptTokens(u) + u.outputTokens;
}

/** `12k in · 1.4k out` (empty when nothing was used). */
export function usageLine(u: Usage): string {
  if (totalTokens(u) === 0) return "";
  return `${fmtTokens(promptTokens(u))} in · ${fmtTokens(u.outputTokens)} out`;
}

/** Share of prompt tokens served from the provider cache (0..1). */
export function cacheShare(u: Usage): number {
  const prompt = promptTokens(u);
  return prompt > 0 ? u.cacheReadTokens / prompt : 0;
}

export function sumUsage(items: Usage[]): Usage {
  return items.reduce<Usage>(
    (acc, u) => ({
      inputTokens: acc.inputTokens + u.inputTokens,
      outputTokens: acc.outputTokens + u.outputTokens,
      cacheReadTokens: acc.cacheReadTokens + u.cacheReadTokens,
      cacheWriteTokens: acc.cacheWriteTokens + u.cacheWriteTokens,
      reasoningTokens: acc.reasoningTokens + u.reasoningTokens,
    }),
    { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0, reasoningTokens: 0 },
  );
}

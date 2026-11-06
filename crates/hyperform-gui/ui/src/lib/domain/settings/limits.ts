/** Numeric settings and the ranges the config loader clamps them to. */

export interface NumberRange {
  min: number;
  max?: number;
  integer: boolean;
}

const U32_MAX = 4_294_967_295;

export const LIMITS = {
  maxOutputTokens: { min: 256, max: 200_000, integer: true },
  contextWindow: { min: 1, max: U32_MAX, integer: true },
  compactAtPercent: { min: 50, max: 99, integer: true },
  keepRecentToolResults: { min: 0, max: U32_MAX, integer: true },
  repoMapChars: { min: 0, max: U32_MAX, integer: true },
  maxContinuations: { min: 0, max: 10, integer: true },
  maxConcurrent: { min: 1, max: U32_MAX, integer: true },
  maxDepth: { min: 0, max: U32_MAX, integer: true },
  maxTurns: { min: 1, max: U32_MAX, integer: true },
  sessionCostCapUsd: { min: 0, integer: false },
  timeoutSecs: { min: 1, max: Number.MAX_SAFE_INTEGER, integer: true },
  decisionTimeoutMs: { min: 50, max: 10_000, integer: true },
  decisionMaxLen: { min: 128, max: 8_192, integer: true },
  decisionMaxBatch: { min: 1, max: 64, integer: true },
  probability: { min: 0, max: 1, integer: false },
} satisfies Record<string, NumberRange>;

export type NumberResult = { ok: true; value: number | null } | { ok: false; error: string };

function rangeText(range: NumberRange): string {
  const kind = range.integer ? "a whole number" : "a number";
  if (range.max !== undefined && range.max < U32_MAX) return `${kind} from ${range.min} to ${range.max}`;
  return `${kind} of at least ${range.min}`;
}

/** Parses a number field; blank is null when the setting is optional. */
export function parseNumber(text: string, range: NumberRange, optional = false): NumberResult {
  const trimmed = text.trim();
  if (!trimmed) {
    return optional ? { ok: true, value: null } : { ok: false, error: `Enter ${rangeText(range)}.` };
  }
  const value = Number(trimmed);
  const outside = value < range.min || (range.max !== undefined && value > range.max);
  if (!Number.isFinite(value) || outside || (range.integer && !Number.isInteger(value))) {
    return { ok: false, error: `Enter ${rangeText(range)}.` };
  }
  return { ok: true, value };
}

import type { ContextBreakdown } from "../protocol/ContextBreakdown";

export interface CtxSlice {
  id: string;
  label: string;
  tokens: number;
  color: string;
}

export interface CtxMeter {
  slices: CtxSlice[];
  used: number;
  max: number;
  remaining: number;
  pct: number;
  level: "ok" | "warn" | "danger";
  /** True when only the total is known (no `/context` breakdown yet). */
  totalOnly: boolean;
}

const LAYERS: Array<{ id: keyof ContextBreakdown; label: string; color: string }> = [
  { id: "system", label: "System prompt", color: "var(--layer-system)" },
  { id: "tools", label: "Tool definitions", color: "var(--layer-tools)" },
  { id: "instructions", label: "Instructions", color: "var(--layer-instructions)" },
  { id: "messages", label: "Conversation", color: "var(--layer-messages)" },
];

export function contextMeter(input: {
  contextTokens: number;
  contextLimit: number;
  breakdown: ContextBreakdown | null;
}): CtxMeter {
  const { breakdown } = input;
  const used = Math.max(0, input.contextTokens || breakdown?.total || 0);
  const max = Math.max(1, input.contextLimit || breakdown?.limit || 0);
  let slices: CtxSlice[];
  if (breakdown && breakdown.total > 0) {
    const scale = used > 0 ? used / breakdown.total : 1;
    slices = LAYERS.map((l) => ({
      id: l.id,
      label: l.label,
      color: l.color,
      tokens: Math.round(breakdown[l.id] * scale),
    })).filter((s) => s.tokens > 0);
  } else {
    slices = used > 0 ? [{ id: "context", label: "Context", tokens: used, color: "var(--layer-messages)" }] : [];
  }
  const pct = Math.min(100, Math.round((used / max) * 100));
  return {
    slices,
    used,
    max,
    remaining: Math.max(0, max - used),
    pct,
    level: pct >= 85 ? "danger" : pct >= 65 ? "warn" : "ok",
    totalOnly: !breakdown,
  };
}

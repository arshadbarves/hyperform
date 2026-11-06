import type { DecisionRecord, SessionDecisions } from "../commands/engine";
import { fmtTokens } from "../util";

/**
 * The Context tab's Decisions section: what the running decision features
 * asked, what came back and what the engine did. It reads only the
 * generic record fields, so a new feature needs no change here.
 */

export type DecisionTone = "acted" | "shadow" | "fallback";

export interface DecisionLine {
  seq: number;
  title: string;
  detail: string;
  /** Why the engine overrode the proposal, if it did. */
  why: string | null;
  tone: DecisionTone;
}

const MODE_LABEL = { off: "Off", shadow: "Shadow", on: "On" } as const;

/** The folded section's one-line hint. */
export function decisionsHint(d: SessionDecisions): string {
  const features = `${d.features.length} ${d.features.length === 1 ? "feature" : "features"}`;
  const asked = `${d.summary.count} ${d.summary.count === 1 ? "decision" : "decisions"}`;
  return d.summary.fallbacks ? `${features} · ${asked} · ${d.summary.fallbacks} fell back` : `${features} · ${asked}`;
}

/** Label and value pairs for the section's facts. */
export function decisionFacts(d: SessionDecisions): { label: string; value: string }[] {
  const facts = [
    { label: "Running", value: d.features.map((f) => `${f.title} (${MODE_LABEL[f.mode]})`).join(", ") || "Nothing" },
    {
      label: "Answered by",
      value:
        d.provider === "hybrid"
          ? "The decision model; when it cannot answer, the feature keeps today's behavior."
          : "Rules only: no decision model is connected, so every feature keeps today's behavior.",
    },
  ];
  const { p50Ms, p95Ms, tokensSaved, wouldSaveTokens } = d.summary;
  if (p50Ms !== null && p95Ms !== null) {
    facts.push({ label: "Speed", value: `Half answered within ${p50Ms} ms, 95% within ${p95Ms} ms` });
  }
  if (tokensSaved || wouldSaveTokens) {
    const parts = [];
    if (tokensSaved) parts.push(`${fmtTokens(tokensSaved)} saved`);
    if (wouldSaveTokens) parts.push(`${fmtTokens(wouldSaveTokens)} would have been saved in shadow`);
    facts.push({ label: "Tokens", value: parts.join("; ") });
  }
  return facts;
}

function percent(confidence: number | null): string {
  return confidence === null ? "" : ` (${Math.round(confidence * 100)}%)`;
}

/** One decision as a title, a detail line and why it was overridden. */
export function decisionLine(record: DecisionRecord, titles: Readonly<Record<string, string>>): DecisionLine {
  const feature = titles[record.feature] ?? record.feature;
  const answer = record.answer === null ? "no answer" : `answered ${record.answer}${percent(record.confidence)}`;
  const parts = [answer];
  if (record.fallback) parts.push(`fell back: ${record.fallback}`);
  parts.push(record.shadow ? `${record.outcome} (shadow, nothing changed)` : record.outcome);
  if (record.tokensSaved) parts.push(`${fmtTokens(record.tokensSaved)} tokens`);
  parts.push(record.cached ? "cached" : `${record.latencyMs} ms`);
  return {
    seq: record.seq,
    title: `${feature} · ${record.question.replaceAll("_", " ")}`,
    detail: parts.join(" · "),
    why: record.overrideReason,
    tone: record.fallback ? "fallback" : record.shadow ? "shadow" : "acted",
  };
}

/** Feature id to title, from the features the session runs. */
export function featureTitles(d: SessionDecisions): Record<string, string> {
  return Object.fromEntries(d.features.map((f) => [f.id, f.title]));
}

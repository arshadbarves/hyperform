import { describe, expect, it } from "vitest";
import type { DecisionRecord, SessionDecisions } from "../commands/engine";
import { decisionFacts, decisionLine, decisionsHint, featureTitles } from "./decisionTrace";

const record = (overrides: Partial<DecisionRecord>): DecisionRecord => ({
  seq: 3,
  atMs: 0,
  feature: "decisions_compaction",
  question: "compaction_relevant",
  shadow: false,
  provider: "systemone",
  inputFingerprint: "abc",
  answer: "yes",
  confidence: 0.92,
  latencyMs: 41,
  cached: false,
  fallback: null,
  outcome: "kept",
  overrideReason: null,
  tokensSaved: null,
  ...overrides,
});

const decisions = (overrides: Partial<SessionDecisions> = {}): SessionDecisions => ({
  features: [{ id: "decisions_compaction", title: "Relevance-aware compaction", mode: "shadow" }],
  provider: "hybrid",
  summary: { count: 12, shadow: 12, fallbacks: 1, p50Ms: 40, p95Ms: 120, tokensSaved: 0, wouldSaveTokens: 3400 },
  records: [],
  ...overrides,
});

describe("decision trace view", () => {
  it("sums up the session on one line", () => {
    expect(decisionsHint(decisions())).toBe("1 feature · 12 decisions · 1 fell back");
    const quiet = decisions({ summary: { ...decisions().summary, count: 1, fallbacks: 0 } });
    expect(decisionsHint(quiet)).toBe("1 feature · 1 decision");
  });

  it("lists what runs, who answers, speed and tokens", () => {
    const facts = Object.fromEntries(decisionFacts(decisions()).map((f) => [f.label, f.value]));
    expect(facts.Running).toBe("Relevance-aware compaction (Shadow)");
    expect(facts["Answered by"]).toContain("decision model");
    expect(facts.Speed).toBe("Half answered within 40 ms, 95% within 120 ms");
    expect(facts.Tokens).toBe("3.4k would have been saved in shadow");
    const rules = decisionFacts(decisions({ provider: "rules", summary: { ...decisions().summary, p50Ms: null, p95Ms: null, wouldSaveTokens: 0 } }));
    expect(rules.map((f) => f.label)).toEqual(["Running", "Answered by"]);
    expect(rules[1]!.value).toContain("Rules only");
  });

  it("describes any feature's record from its generic fields", () => {
    const titles = featureTitles(decisions());
    expect(decisionLine(record({}), titles)).toEqual({
      seq: 3,
      title: "Relevance-aware compaction · compaction relevant",
      detail: "answered yes (92%) · kept · 41 ms",
      why: null,
      tone: "acted",
    });
    const shadow = decisionLine(record({ shadow: true, outcome: "cleared", tokensSaved: 1200, cached: true }), titles);
    expect(shadow.detail).toBe("answered yes (92%) · cleared (shadow, nothing changed) · 1.2k tokens · cached");
    expect(shadow.tone).toBe("shadow");
    const fallback = decisionLine(record({ feature: "decisions_new", answer: null, confidence: null, fallback: "timeout", outcome: "unchanged" }), titles);
    expect(fallback.title.startsWith("decisions_new")).toBe(true);
    expect(fallback.detail).toBe("no answer · fell back: timeout · unchanged · 41 ms");
    expect(fallback.tone).toBe("fallback");
    expect(decisionLine(record({ overrideReason: "file named by the user" }), titles).why).toBe("file named by the user");
  });
});

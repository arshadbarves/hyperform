import { describe, expect, it } from "vitest";
import type { DecisionModelTest } from "../../commands/settings";
import type { FeatureSpec } from "../../protocol/config/FeatureSpec";
import type { Settings } from "../../protocol/config/Settings";
import { endpointProblem, testSummary } from "./decisionModel";
import { decisionsInUse, featureEntries, featureMode, featureModeOptions, listedFeatures } from "./features";

const spec = (overrides: Partial<FeatureSpec>): FeatureSpec => ({
  id: "decisions_compaction",
  title: "Relevance-aware compaction",
  summary: "Clear the results the task no longer needs first.",
  stage: "experimental",
  group: "decisions",
  supportsShadow: true,
  available: true,
  requires: null,
  owner: "hyperform core",
  graduationCriteria: "fewer tokens",
  ...overrides,
});

const settings = (experimental: Settings["experimental"]) => ({ experimental }) as Settings;

const catalog = [
  spec({}),
  spec({ id: "decisions_risk", title: "Risk review", available: false }),
  spec({ id: "decisions_loop_guard", title: "Loop guard", supportsShadow: false }),
];

describe("experimental features", () => {
  it("lists only the features built in this version", () => {
    expect(listedFeatures(catalog).map((s) => s.id)).toEqual(["decisions_compaction", "decisions_loop_guard"]);
    expect(listedFeatures([spec({ stage: "stable" })])).toEqual([]);
  });

  it("offers shadow only where the feature supports it", () => {
    expect(featureModeOptions(catalog[0]!).map((o) => o.value)).toEqual(["off", "shadow", "on"]);
    expect(featureModeOptions(catalog[2]!).map((o) => o.value)).toEqual(["off", "on"]);
  });

  it("reads a missing mode as off", () => {
    expect(featureMode(settings({}), "decisions_compaction")).toBe("off");
    expect(featureMode(settings({ decisions_compaction: "shadow" }), "decisions_compaction")).toBe("shadow");
  });

  it("shows the decision model card while a listed decisions feature runs", () => {
    expect(decisionsInUse(settings({}), catalog)).toBe(false);
    expect(decisionsInUse(settings({ decisions_compaction: "off" }), catalog)).toBe(false);
    expect(decisionsInUse(settings({ decisions_compaction: "shadow" }), catalog)).toBe(true);
    expect(decisionsInUse(settings({ decisions_risk: "on" }), catalog)).toBe(false);
  });

  it("makes listed features and the visible card searchable", () => {
    const off = featureEntries(catalog, settings({}));
    expect(off.map((e) => e.key)).toEqual(["experimental.decisions_compaction", "experimental.decisions_loop_guard"]);
    expect(off[0]).toMatchObject({ tab: "experimental", title: "Relevance-aware compaction", group: null });
    expect(off[0]!.words).toContain("no longer needs");
    const on = featureEntries(catalog, settings({ decisions_loop_guard: "on" }));
    expect(on.map((e) => e.key)).toContain("decisions.endpoint");
    expect(featureEntries(catalog, null)).toHaveLength(2);
  });
});

const test = (overrides: Partial<DecisionModelTest>): DecisionModelTest => ({
  ok: true,
  latencyMs: 40,
  answer: "yes",
  confidence: 0.97,
  error: null,
  endpoint: "http://127.0.0.1:8000/v1/systemone",
  warmingUp: false,
  timeoutMs: 250,
  ...overrides,
});

describe("decision model card", () => {
  it("accepts http and https endpoints only", () => {
    expect(endpointProblem("http://127.0.0.1:8000")).toBeNull();
    expect(endpointProblem("https://laya.example.com")).toBeNull();
    expect(endpointProblem("ftp://host")).not.toBeNull();
    expect(endpointProblem("localhost:8000")).not.toBeNull();
  });

  it("reports a working, slow, wrong or unreachable model", () => {
    expect(testSummary(test({}))).toEqual({ tone: "ok", text: "Connected. Answered in 40 ms (97% confident)." });
    expect(testSummary(test({ latencyMs: 900 })).text).toContain("slower than the 250 ms timeout");
    expect(testSummary(test({ answer: "no" })).tone).toBe("warn");
    const down = testSummary(test({ ok: false, answer: null, error: "the decision server is unreachable" }));
    expect(down).toEqual({ tone: "error", text: "Not connected: the decision server is unreachable." });
    expect(testSummary(test({ ok: false, error: "timed out", warmingUp: true })).text).toContain("still be loading");
  });
});

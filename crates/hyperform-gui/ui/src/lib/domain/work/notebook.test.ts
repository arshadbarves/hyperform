import { describe, expect, it } from "vitest";
import { entry, task } from "./fixtures";
import { causeLabel, entryLine, evidenceLabel, linkedEntries, notebook, notebookSummary } from "./notebook";

const decision = entry(
  "d1",
  { type: "designDecision", title: "Queue in IndexedDB", choice: "IndexedDB", alternatives: ["localStorage", "SQLite"], reason: "Size limits" },
  { tasks: ["t1"], createdAt: 10 },
);
const discovery = entry("f1", { type: "discovery", text: "C is generated", source: "build.rs" }, { createdAt: 20, tasks: ["t1"] });
const extracted = entry(
  "f2",
  { type: "discovery", text: "Retries back off", source: null },
  { createdAt: 30, provenance: "compaction", status: { type: "proposed" } },
);
const dismissed = entry("f3", { type: "discovery", text: "Gone", source: null }, { status: { type: "dismissed" } });
const change = entry("c1", {
  type: "planChange",
  cause: "discovery",
  source: "f1",
  original: "Modify C",
  new: "Modify D",
  reason: "C is generated",
  evidence: [{ type: "note", text: "generation header" }],
  affected: ["t1"],
});
const evidence = entry("e1", { type: "evidence", taskId: "t1", criterionId: null, evidence: { type: "note", text: "ok" } });

const entries = Object.fromEntries([decision, discovery, extracted, dismissed, change, evidence].map((e) => [e.id, e]));

describe("notebook", () => {
  it("groups entries by tab, newest first, without dismissed ones or evidence", () => {
    const groups = notebook(entries);
    expect(groups.discoveries.map((e) => e.id)).toEqual(["f2", "f1"]);
    expect(groups.decisions.map((e) => e.id)).toEqual(["d1"]);
    expect(groups.changes.map((e) => e.id)).toEqual(["c1"]);
    expect(notebookSummary(groups)).toBe("Decisions 1 · Discoveries 2 · Changes 1");
    expect(notebookSummary(notebook({}))).toBe("Nothing recorded yet");
  });

  it("expands a decision to its alternatives and reason", () => {
    const line = entryLine(decision, {});
    expect(line.title).toBe("Queue in IndexedDB");
    expect(line.facts).toEqual([
      { label: "Choice", value: "IndexedDB" },
      { label: "Alternatives", value: "localStorage · SQLite" },
      { label: "Reason", value: "Size limits" },
    ]);
  });

  it("tags an extracted entry Unconfirmed with where it came from", () => {
    expect(entryLine(extracted, {})).toMatchObject({ unconfirmed: true, source: "Extracted while compacting" });
    expect(entryLine(discovery, {})).toMatchObject({ unconfirmed: false, source: null });
  });

  it("keeps a web entry marked from web after it is confirmed", () => {
    const web = { ...discovery, provenance: "research" as const };
    expect(entryLine({ ...web, status: { type: "proposed" } }, {})).toMatchObject({
      unconfirmed: true,
      fromWeb: true,
      source: "From web research",
    });
    expect(entryLine(web, {})).toMatchObject({ unconfirmed: false, fromWeb: true, source: null });
    expect(entryLine(discovery, {}).fromWeb).toBe(false);
  });

  it("expands a change to its cause, from and to, evidence and tasks", () => {
    const line = entryLine(change, { t1: task("t1", { title: "Modify the generator" }) });
    expect(line).toMatchObject({ title: "C is generated", tag: "Discovery" });
    expect(line.facts.map((f) => f.label)).toEqual(["From", "To", "Evidence", "Tasks"]);
    expect(line.facts[3].value).toBe("Modify the generator");
  });

  it("tags a resume change Resumed and keeps what resuming found as its title", () => {
    const body = { type: "planChange" as const, cause: "resume" as const, source: null, original: "revision 9", new: "", evidence: [], affected: [] };
    const resumed = entry("r1", { ...body, reason: "Resumed: 1 task was interrupted, the workspace changed outside the plan" });
    expect(entryLine(resumed, {})).toMatchObject({ title: "1 task was interrupted, the workspace changed outside the plan", tag: "Resumed" });
    expect(entryLine(entry("r2", { ...body, reason: "Resumed: the workspace changed outside the plan" }), {}).title).toBe(
      "The workspace changed outside the plan",
    );
    expect(entryLine(entry("r3", { ...body, reason: "" }), {}).title).toBe("Plan resumed");
    expect(causeLabel("resume")).toBe("Resumed");
  });

  it("links the decisions, discoveries and experiments that touch a task", () => {
    expect(linkedEntries(entries, "t1").map((e) => e.id)).toEqual(["f1", "d1"]);
  });

  it("labels evidence", () => {
    const check = { type: "check" as const, sessionId: "S1", recordId: "r1", passed: false, fingerprint: "f", kind: null, label: "cargo test", at: 1 };
    expect(evidenceLabel(check)).toBe("cargo test failed");
    expect(evidenceLabel({ type: "external", url: "https://x", title: "" })).toBe("https://x");
  });
});

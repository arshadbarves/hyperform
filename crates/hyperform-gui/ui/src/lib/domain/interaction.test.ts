import { describe, expect, it } from "vitest";
import type { Question } from "../protocol/Question";
import { elapsed, fmtDuration, plural } from "./format";
import { MODES, modeChips, modeMeta, PLAN, startStyle, styleSettings, togglePlanning } from "./modes";
import {
  buildAnswers,
  canSubmitAnswers,
  emptyDrafts,
  questionTab,
  toggleOption,
  toggleOther,
} from "./questions";
import { checkRecord, turnRecord } from "./testFixtures";
import { todoProgress } from "./todos";
import { checkDetail, checkStatus, outcomeNote, turnEvidence, verificationBadge } from "./verification";

const single: Question = {
  question: "Which database?",
  header: "Database",
  options: [
    { label: "Postgres", description: "Relational" },
    { label: "SQLite", description: null },
  ],
  multiSelect: false,
};
const multi: Question = { ...single, question: "Which features?", header: "", multiSelect: true };

describe("questions", () => {
  it("replaces single picks and toggles multi picks", () => {
    const [draft] = emptyDrafts([single]);
    const picked = toggleOption(toggleOption(draft, single, "Postgres"), single, "SQLite");
    expect(picked.selected).toEqual(["SQLite"]);
    const many = toggleOption(toggleOption(draft, multi, "Postgres"), multi, "SQLite");
    expect(many.selected).toEqual(["Postgres", "SQLite"]);
    expect(toggleOption(many, multi, "Postgres").selected).toEqual(["SQLite"]);
  });

  it("uses Other as a free-text answer", () => {
    const [draft] = emptyDrafts([single]);
    const other = { ...toggleOther(toggleOption(draft, single, "SQLite"), single), other: " DuckDB " };
    expect(other.selected).toEqual([]);
    expect(buildAnswers([single], [other])).toEqual([{ question: "Which database?", answers: ["DuckDB"] }]);
  });

  it("requires an answer for every question", () => {
    const drafts = emptyDrafts([single, multi]);
    expect(canSubmitAnswers([single, multi], drafts)).toBe(false);
    const answered = [toggleOption(drafts[0], single, "SQLite"), toggleOption(drafts[1], multi, "Postgres")];
    expect(canSubmitAnswers([single, multi], answered)).toBe(true);
    expect(canSubmitAnswers([], [])).toBe(false);
  });

  it("labels tabs with the header or a clipped question", () => {
    expect(questionTab(single, 0)).toBe("Database");
    expect(questionTab(multi, 1)).toBe("Which features?");
    expect(questionTab({ ...multi, question: "" }, 2)).toBe("Question 3");
  });
});

describe("todoProgress", () => {
  it("counts done items and prefers the in-progress label", () => {
    const progress = todoProgress([
      { content: "Write code", activeForm: "Writing code", status: "completed" },
      { content: "Run tests", activeForm: "Running tests", status: "in_progress" },
      { content: "Ship", activeForm: "Shipping", status: "pending" },
    ]);
    expect(progress).toMatchObject({ done: 1, total: 3, currentLabel: "Running tests", allDone: false });
  });

  it("falls back to the next pending item", () => {
    const progress = todoProgress([{ content: "Ship", activeForm: "", status: "pending" }]);
    expect(progress.currentLabel).toBe("Ship");
    expect(todoProgress(undefined)).toMatchObject({ total: 0, current: null, allDone: false });
  });
});

describe("verification", () => {
  it("maps outcomes to badges", () => {
    expect(verificationBadge({ status: "verified", checks: [] })).toMatchObject({ label: "Verified", tone: "ok" });
    expect(verificationBadge({ status: "unverified", reason: "no checks" })).toMatchObject({
      label: "Unverified",
      reason: "no checks",
    });
    expect(verificationBadge({ status: "failed", reason: "tests" }).tone).toBe("err");
    expect(verificationBadge({ status: "notApplicable" }).label).toBe("Not applicable");
  });

  it("only calls out unusual turn endings", () => {
    expect(outcomeNote({ type: "completed" })).toBeNull();
    expect(outcomeNote({ type: "budgetExhausted", reason: "max turns" })?.label).toBe("Stopped · max turns");
    expect(outcomeNote({ type: "failed", message: "boom" })?.tone).toBe("err");
  });

  it("collects evidence for a turn", () => {
    const checks = [
      checkRecord({ recordId: "r1", startedAt: 5 }),
      checkRecord({ recordId: "r2", startedAt: 500 }),
    ];
    const verified = turnRecord({ verification: { status: "verified", checks: ["r2"] } });
    expect(turnEvidence(verified, checks).map((c) => c.recordId)).toEqual(["r2"]);
    const failed = turnRecord({ verification: { status: "failed", reason: "x" }, startedAt: 0, finishedAt: 100 });
    expect(turnEvidence(failed, checks).map((c) => c.recordId)).toEqual(["r1"]);
  });

  it("summarizes a check", () => {
    expect(checkStatus(checkRecord())).toEqual({ label: "passed", tone: "ok" });
    expect(checkStatus(checkRecord({ passed: false, exitCode: 101 })).label).toBe("exit 101");
    expect(checkStatus(checkRecord({ timedOut: true })).label).toBe("timed out");
    expect(checkDetail(checkRecord())).toBe("12 passed · 1 skipped · 1.2s");
  });
});

describe("modes and formatting", () => {
  it("offers three levels and flags bypass; plan is not a level", () => {
    expect(MODES.map((m) => m.label)).toEqual(["Ask", "Auto-accept edits", "Bypass"]);
    expect(modeMeta("bypass").warning).toBeTruthy();
    expect(MODES.filter((m) => m.warning).map((m) => m.id)).toEqual(["bypass"]);
  });

  it("shows the removable Plan chip only while planning, beside any level", () => {
    expect(modeChips(false, "default")).toEqual({ plan: null, level: modeMeta("default") });
    expect(modeChips(true, "bypass")).toEqual({ plan: PLAN, level: modeMeta("bypass") });
    expect(modeChips(true, "acceptEdits").level.label).toBe("Auto-accept edits");
  });

  it("toggles plan mode (Shift+Tab) without touching the level", () => {
    expect(togglePlanning(false)).toBe(true);
    expect(togglePlanning(true)).toBe(false);
  });

  it("maps onboarding's Plan first to plan mode at Ask", () => {
    expect(styleSettings("plan")).toEqual({ mode: "default", planning: true });
    expect(styleSettings("acceptEdits")).toEqual({ mode: "acceptEdits", planning: false });
    expect(startStyle("acceptEdits", true)).toBe("plan");
    expect(startStyle("default", false)).toBe("default");
    expect(startStyle("bypass", false)).toBeNull();
  });

  it("formats durations", () => {
    expect(fmtDuration(850)).toBe("850ms");
    expect(fmtDuration(4200)).toBe("4.2s");
    expect(fmtDuration(185_000)).toBe("3m 05s");
    expect(fmtDuration(4_320_000)).toBe("1h 12m");
    expect(fmtDuration(null)).toBe("");
    expect(elapsed(1000, null, 3500)).toBe("2.5s");
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(2, "file")).toBe("2 files");
  });
});

import { describe, expect, it } from "vitest";
import type { PlanChange } from "../../protocol/PlanChange";
import { entry, task } from "./fixtures";
import { isResumeChange, resumeNote, resumeSummary } from "./resume";

const change = (over: Partial<PlanChange>) =>
  entry("c1", { type: "planChange", cause: "resume", source: null, original: "", new: "", reason: "", evidence: [], affected: [], ...over });

describe("resuming a plan", () => {
  it("tells the resume change from other plan changes", () => {
    expect(isResumeChange(change({}))).toBe(true);
    expect(isResumeChange(change({ cause: "discovery" }))).toBe(false);
    expect(isResumeChange(entry("d", { type: "discovery", text: "x", source: null }))).toBe(false);
  });

  it("keeps the engine's summary without its lead", () => {
    expect(resumeSummary(change({ reason: "Resumed: 2 tasks were interrupted, 1 file changed outside the plan" }))).toBe(
      "2 tasks were interrupted, 1 file changed outside the plan",
    );
    expect(resumeSummary(change({ new: "Nothing changed" }))).toBe("Nothing changed");
  });

  it("says where an interrupted task picks up, only while it waits to start again", () => {
    const pointer = { sessionId: "S1", agentId: "agt_2", worktree: "/repo/.z-engine/worktrees/agt_2", note: " Stopped in step 3 " };
    expect(resumeNote(task("a", { resume: pointer }))).toEqual({
      text: "Interrupted; its helper's work is kept in a worktree",
      note: "Stopped in step 3",
      worktree: pointer.worktree,
      agentId: "agt_2",
      sessionId: "S1",
    });
    expect(resumeNote(task("a", { resume: { ...pointer, worktree: null, note: "" } }))?.text).toBe(
      "Interrupted; it picks up from its helper's transcript",
    );
    expect(resumeNote(task("a", { resume: { ...pointer, agentId: "main", worktree: null } }))).toMatchObject({
      text: "Interrupted; it picks up where it stopped",
      agentId: null,
    });
    expect(resumeNote(task("a", { resume: pointer, status: { type: "active" } }))).toBeNull();
    expect(resumeNote(task("a"))).toBeNull();
  });
});

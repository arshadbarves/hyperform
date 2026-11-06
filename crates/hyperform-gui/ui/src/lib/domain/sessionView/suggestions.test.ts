import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import type { Suggestion } from "../../protocol/Suggestion";
import { info, snapshot, turnRecord } from "../testFixtures";
import { reduce } from "./reduce";
import { currentSuggestions } from "./suggestions";
import { emptyView, type SessionView } from "./types";

const plan: Suggestion = { suggestionId: "decisions_plan_suggest:ab", kind: { type: "planFirst" } };
const review: Suggestion = {
  suggestionId: "decisions_review_suggest:cd",
  kind: { type: "review", areas: ["authentication"], paths: ["src/auth.rs"] },
};

function run(view: SessionView, ...events: Event[]): SessionView {
  return events.reduce((v, event, index) => reduce(v, event, index), view);
}

const suggested = (suggestion: Suggestion): Event => ({ type: "suggested", suggestion });
const started = (turnId: string, messageId: string): Event => ({ type: "turnStarted", turnId, messageId });
const finished = (turnId: string, messageId: string): Event => ({
  type: "turnFinished",
  turn: turnRecord({ turnId, messageId }),
});

describe("suggestions", () => {
  it("show through their turn and hide once the next turn starts", () => {
    let view = run(emptyView("S1"), suggested(plan), started("t1", "u1"));
    expect(currentSuggestions(view)).toEqual([plan]);
    view = run(view, suggested(review), finished("t1", "u1"));
    expect(currentSuggestions(view)).toEqual([plan, review]);
    view = run(view, started("t2", "u2"));
    expect(currentSuggestions(view)).toEqual([]);
  });

  it("are dropped when resolved and replaced when offered again", () => {
    let view = run(emptyView("S1"), suggested(plan), suggested(plan), suggested(review));
    expect(view.suggestions).toHaveLength(2);
    view = run(view, { type: "suggestionResolved", suggestionId: plan.suggestionId, accepted: true });
    expect(currentSuggestions(view)).toEqual([review]);
    const same = run(view, { type: "suggestionResolved", suggestionId: "unknown", accepted: false });
    expect(same.suggestions).toBe(view.suggestions);
  });

  it("offer plan mode only at the Default level outside plan mode", () => {
    const view = run(emptyView("S1"), suggested(plan), { type: "planningChanged", planning: true });
    expect(currentSuggestions(view)).toEqual([]);
    const building = run(view, { type: "planningChanged", planning: false });
    expect(currentSuggestions(building)).toEqual([plan]);
    expect(currentSuggestions(run(building, { type: "modeChanged", mode: "acceptEdits" }))).toEqual([]);
  });

  it("survive a snapshot", () => {
    const view = run(emptyView("S1"), suggested(review));
    const reloaded = reduce(view, { type: "snapshot", snapshot: snapshot({ info: info() }) }, 9);
    expect(currentSuggestions(reloaded)).toEqual([review]);
  });
});

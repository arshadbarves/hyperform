import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import type { RouteInfo } from "../../protocol/RouteInfo";
import { buildTimeline } from "../timeline/turns";
import { assistant, snapshot, user } from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";

const route: RouteInfo = { agentId: "main", effort: "high", model: null, reason: "complex task, confidence 0.91" };
const chosen: Event = { type: "routeChosen", route };

function withMessages(...ids: string[]): SessionView {
  return { ...emptyView("S1"), messages: ids.map((id) => user(id, id)) };
}

describe("routeChosen", () => {
  it("anchors the route after the last message", () => {
    const view = reduce(withMessages("u1", "u2"), chosen, 1000);
    expect(view.routes).toEqual([{ id: 1, route, afterMessageId: "u2", at: 1000 }]);
    expect(view.nextLocalId).toBe(2);
  });

  it("survives a snapshot while its message does", () => {
    const view = reduce(withMessages("u1"), chosen, 1000);
    const kept = reduce(view, { type: "snapshot", snapshot: snapshot({ messages: [user("u1", "u1")] }) }, 2000);
    expect(kept.routes).toHaveLength(1);
    const rewound = reduce(view, { type: "snapshot", snapshot: snapshot({ messages: [] }) }, 2000);
    expect(rewound.routes).toEqual([]);
  });

  it("shows the chip in the turn it was chosen for, before the reply", () => {
    const view = reduce(withMessages("u1"), chosen, 1000);
    const messages = [...view.messages, assistant("a1", "Done.")];
    const [turn] = buildTimeline({ messages, routes: view.routes });
    expect(turn.items.map((item) => item.kind)).toEqual(["route", "text"]);
  });
});

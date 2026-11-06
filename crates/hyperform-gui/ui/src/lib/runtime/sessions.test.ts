import { describe, expect, it } from "vitest";
import { approval, envelope, snapshot, turnRecord, user } from "../domain/testFixtures";
import { sessions } from "./sessions.svelte";

describe("sessions store", () => {
  it("tracks every session, the active one and background unread turns", () => {
    sessions.activate("A");
    expect(sessions.hydrating).toBe(true);

    sessions.apply(envelope({ type: "snapshot", snapshot: snapshot({ messages: [user("u1", "hi")] }) }, 0, "A"));
    expect(sessions.hydrating).toBe(false);
    expect(sessions.active?.messages).toHaveLength(1);

    const effects = sessions.apply(envelope({ type: "approvalRequested", request: approval() }, 0, "B"));
    expect(effects).toEqual([]);
    expect(sessions.activity).toEqual({ B: "approval" });

    sessions.apply(envelope({ type: "turnFinished", turn: turnRecord() }, 1, "B"));
    expect(Object.keys(sessions.unread)).toEqual(["B"]);
    sessions.activate("B");
    expect(sessions.unread).toEqual({});
    expect(sessions.active?.sessionId).toBe("B");
  });

  it("returns the same activity object while nothing changed", () => {
    const before = sessions.activity;
    sessions.apply(envelope({ type: "titleChanged", title: "x" }, 5, "A"));
    expect(sessions.activity).toBe(before);
  });

  it("applies local cards without consuming engine sequence numbers", () => {
    const seq = sessions.view("A")?.lastSeq;
    sessions.applyLocal("A", { type: "commandOutput", name: "help", markdown: "**keys**" });
    expect(sessions.view("A")?.outputs.at(-1)?.name).toBe("help");
    expect(sessions.view("A")?.lastSeq).toBe(seq);
  });

  it("forgets deleted sessions and clears the selection", () => {
    sessions.activate("A");
    sessions.forget("A");
    expect(sessions.activeId).toBeNull();
    expect(sessions.view("A")).toBeNull();
  });
});

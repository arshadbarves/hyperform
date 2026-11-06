import { describe, expect, it } from "vitest";
import type { Event } from "../../protocol/Event";
import type { TaskViewInfo } from "../../protocol/TaskViewInfo";
import { splitTrailingCompactions } from "../timeline/groups";
import { taskViewDivider } from "../timeline/taskViewDivider";
import { buildTimeline } from "../timeline/turns";
import { assistant, snapshot, user } from "../testFixtures";
import { reduce } from "./reduce";
import { emptyView, type SessionView } from "./types";

const info: TaskViewInfo = { boundary: "u2", setAside: 12, tokens: 48_000, restored: false, createdAt: 10 };
const applied = (view: TaskViewInfo): Event => ({ type: "taskViewApplied", view });

function chat(): SessionView {
  return { ...emptyView("S1"), messages: [user("u1", "first", 1), assistant("a1", "done", 2), user("u2", "next", 9)] };
}

describe("taskViewApplied", () => {
  it("adds the view, and a restore replaces it in place", () => {
    const view = reduce(chat(), applied(info), 1000);
    expect(view.taskViews).toEqual([info]);
    const restored = reduce(view, applied({ ...info, restored: true }), 2000);
    expect(restored.taskViews).toEqual([{ ...info, restored: true }]);
  });

  it("comes back from the snapshot when the chat reopens", () => {
    const view = reduce(emptyView("S1"), { type: "snapshot", snapshot: snapshot({ taskViews: [info] }) }, 0);
    expect(view.taskViews).toEqual([info]);
  });

  it("draws the divider before the new task, after the earlier turn's actions", () => {
    const view = reduce(chat(), applied(info), 1000);
    const turns = buildTimeline({ messages: view.messages, taskViews: view.taskViews });
    expect(turns.map((turn) => turn.key)).toEqual(["u1", "u2"]);
    const { body, after } = splitTrailingCompactions(turns[0].items);
    expect(body.map((item) => item.kind)).toEqual(["text"]);
    expect(after).toEqual([{ kind: "taskView", key: "taskView:u2", view: info, latest: true }]);
  });

  it("offers the way back only on the newest view, before any compaction", () => {
    const older = { ...info, boundary: "u1", createdAt: 5 };
    const both = buildTimeline({ messages: chat().messages, taskViews: [older, info] });
    const latest = both.flatMap((turn) => turn.items).flatMap((item) => (item.kind === "taskView" ? [item.latest] : []));
    expect(latest).toEqual([false, true]);
    const marker = { keepFrom: null, summary: "s", tokensBefore: 1, tokensAfter: 1, createdAt: 20 };
    const compacted = buildTimeline({ messages: chat().messages, taskViews: [info], compactions: [marker] });
    const item = compacted.flatMap((turn) => turn.items).find((i) => i.kind === "taskView");
    expect(item).toMatchObject({ latest: false });
  });
});

describe("taskViewDivider", () => {
  it("says what was set aside and offers the full history", () => {
    expect(taskViewDivider(info, true)).toEqual({
      label: "12 earlier exchanges (48k tokens) set aside for this task",
      canRestore: true,
    });
    expect(taskViewDivider({ ...info, setAside: 1, tokens: 900 }, false)).toEqual({
      label: "1 earlier exchange (900 tokens) set aside for this task",
      canRestore: false,
    });
  });

  it("says the full history is back once restored", () => {
    expect(taskViewDivider({ ...info, restored: true }, true)).toEqual({
      label: "Full history included for this task",
      canRestore: false,
    });
  });
});

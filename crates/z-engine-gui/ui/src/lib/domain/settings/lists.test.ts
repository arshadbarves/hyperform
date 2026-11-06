import { describe, expect, it } from "vitest";
import type { CheckConfig } from "../../protocol/config/CheckConfig";
import { checkError, checkForFile, emptyCheck, layerChecks, normalizeCheck, unknownAutoChecks } from "./checks";
import { emptyHook, HOOK_EVENTS, hookError, hookForEvent, layerHooks, normalizeHook } from "./hooks";
import { appendItem, moveItem, removeItem, replaceItem } from "./listEdits";

describe("list edits", () => {
  const list = ["a", "b", "c"];

  it("append, replace and remove without mutating", () => {
    expect(appendItem(list, "d")).toEqual(["a", "b", "c", "d"]);
    expect(replaceItem(list, 1, "B")).toEqual(["a", "B", "c"]);
    expect(replaceItem(list, 5, "x")).toEqual(list);
    expect(removeItem(list, 0)).toEqual(["b", "c"]);
    expect(list).toEqual(["a", "b", "c"]);
  });

  it("reorders within bounds only", () => {
    expect(moveItem(list, 0, 1)).toEqual(["b", "a", "c"]);
    expect(moveItem(list, 2, -2)).toEqual(["c", "a", "b"]);
    expect(moveItem(list, 0, -1)).toEqual(list);
    expect(moveItem(list, 2, 1)).toEqual(list);
  });
});

describe("hooks", () => {
  it("lists the nine events and which take a matcher", () => {
    expect(HOOK_EVENTS.map((e) => e.name)).toEqual([
      "SessionStart",
      "UserPromptSubmit",
      "PreToolUse",
      "PostToolUse",
      "Stop",
      "SubagentStop",
      "PreCompact",
      "Notification",
      "SessionEnd",
    ]);
    expect(HOOK_EVENTS.filter((e) => e.matcher).map((e) => e.name)).toEqual([
      "SessionStart",
      "PreToolUse",
      "PostToolUse",
      "PreCompact",
    ]);
  });

  it("reads a layer's hooks with the loader's defaults", () => {
    const raw = { hooks: { PreToolUse: [{ command: "./check.sh", matcher: "Bash" }, { command: "x", timeout_secs: 5, matcher: "" }] } };
    expect(layerHooks(raw, "PreToolUse")).toEqual([
      { matcher: "Bash", command: "./check.sh", timeout_secs: 60 },
      { matcher: null, command: "x", timeout_secs: 5 },
    ]);
    expect(layerHooks(raw, "Stop")).toEqual([]);
    expect(normalizeHook("junk")).toEqual(emptyHook());
  });

  const event = (name: string) => HOOK_EVENTS.find((e) => e.name === name)!;

  it("validates command, timeout and matcher", () => {
    const hook = { matcher: "Bash|Edit", command: "./x.sh", timeout_secs: 60 };
    const pre = event("PreToolUse");
    expect(hookError(hook, pre)).toBeNull();
    expect(hookError({ ...hook, command: " " }, pre)).toContain("command");
    expect(hookError({ ...hook, timeout_secs: 0 }, pre)).toContain("at least 1");
    expect(hookError({ ...hook, matcher: "(" }, pre)).toContain("not a valid");
    expect(hookError({ ...hook, matcher: "(?=Bash)" }, pre)).toContain("lookaround");
    expect(hookError({ ...hook, matcher: "(" }, event("SessionStart"))).toContain("not a valid");
    expect(hookError({ ...hook, matcher: "(" }, event("Stop"))).toBeNull();
  });

  it("keeps the matcher of events the engine matches, drops it elsewhere", () => {
    const hook = { matcher: " resume ", command: " ./x.sh ", timeout_secs: 9 };
    expect(hookForEvent(hook, event("PreToolUse"))).toEqual({ matcher: "resume", command: "./x.sh", timeout_secs: 9 });
    expect(hookForEvent(hook, event("SessionStart"))).toEqual({ matcher: "resume", command: "./x.sh", timeout_secs: 9 });
    expect(hookForEvent({ ...hook, matcher: "auto" }, event("PreCompact"))?.matcher).toBe("auto");
    expect(hookForEvent(hook, event("Stop"))).toEqual({ matcher: null, command: "./x.sh", timeout_secs: 9 });
  });
});

describe("checks", () => {
  const unit: CheckConfig = { id: "unit", label: "", command: "cargo test", kind: "test", cwd: null, timeout_secs: 600 };

  it("reads a layer's checks, accepting the loader's kind spellings", () => {
    const raw = { verification: { checks: [{ id: "u", command: "c", kind: "tests" }, { id: "t", command: "c", kind: "weird", cwd: "ui" }] } };
    expect(layerChecks(raw).map((c) => [c.kind, c.cwd, c.timeout_secs])).toEqual([
      ["test", null, 600],
      ["custom", "ui", 600],
    ]);
    expect(normalizeCheck(null)).toMatchObject({ id: "", kind: "custom" });
  });

  it("requires a unique id and a command", () => {
    expect(checkError(unit, [])).toBeNull();
    expect(checkError({ ...unit, id: "" }, [])).toContain("id");
    expect(checkError({ ...unit, id: "has space" }, [])).toContain("letters");
    expect(checkError(unit, [unit])).toContain("already has");
    expect(checkError({ ...unit, command: "" }, [])).toContain("command");
    expect(checkError({ ...unit, timeout_secs: 0 }, [])).toContain("timeout");
    expect(checkError(emptyCheck(), [])).toContain("id");
  });

  it("writes trimmed checks and leaves out an empty label and cwd", () => {
    expect(checkForFile({ ...unit, id: " unit ", command: " cargo test ", cwd: " " })).toEqual({
      id: "unit",
      command: "cargo test",
      kind: "test",
      cwd: null,
      timeout_secs: 600,
    });
    expect(checkForFile({ ...unit, label: "Unit tests" })).toMatchObject({ label: "Unit tests" });
  });

  it("flags auto checks that name no kind or configured check", () => {
    expect(unknownAutoChecks(["test", "unit", "e2e"], ["unit"])).toEqual(["e2e"]);
  });
});

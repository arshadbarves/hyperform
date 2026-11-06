import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as engine from "./engine";
import * as workspace from "./workspace";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.clearAllMocks());

describe("workspace and context IPC", () => {
  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ["gitSummary", () => workspace.gitSummary("/work/app"), "git_summary", { root: "/work/app" }],
    ["listChangedFiles (active)", () => workspace.listChangedFiles(), "list_changed_files", { root: null }],
    ["listChangedFiles (root)", () => workspace.listChangedFiles("/work/app"), "list_changed_files", { root: "/work/app" }],
    ["diffForFile", () => workspace.diffForFile("src/a.rs", "/work/app"), "diff_for_file", { path: "src/a.rs", root: "/work/app" }],
    ["createWorktree", () => workspace.createWorktree("fix", "/work/app"), "create_worktree", { name: "fix", root: "/work/app" }],
    ["openPath", () => workspace.openPath("/work/app/src/a.rs"), "open_path", { path: "/work/app/src/a.rs" }],
    ["revealPath", () => workspace.revealPath("/work/app"), "reveal_path", { path: "/work/app" }],
    ["contextBreakdown", () => engine.contextBreakdown("s1"), "context_breakdown", { sessionId: "s1" }],
    ["sessionDecisions", () => engine.sessionDecisions("s1", 50), "session_decisions", { sessionId: "s1", limit: 50 }],
  ])("%s invokes %s", async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledWith(command, args);
  });
});

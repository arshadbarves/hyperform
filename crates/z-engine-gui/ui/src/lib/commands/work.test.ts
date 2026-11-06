import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as work from "./work";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.clearAllMocks());

describe("work plan IPC", () => {
  it.each<[string, () => Promise<unknown>, string, Record<string, unknown>]>([
    ["workPlans", () => work.workPlans("/work/app"), "work_plans", { projectRoot: "/work/app" }],
    ["workPacket", () => work.workPacket("s1", "t1"), "work_packet", { sessionId: "s1", taskId: "t1" }],
    ["workWhy", () => work.workWhy("s1", "t1"), "work_why", { sessionId: "s1", taskId: "t1" }],
    ["workParallel", () => work.workParallel("s1"), "work_parallel", { sessionId: "s1" }],
  ])("%s invokes %s", async (_name, call, command, args) => {
    await call();
    expect(invoke).toHaveBeenCalledWith(command, args);
  });
});

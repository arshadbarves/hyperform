import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { summary } from "../domain/work/fixtures";
import { projectPlans } from "./plans.svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invoked = vi.mocked(invoke);

beforeEach(() => {
  invoked.mockReset();
  invoked.mockImplementation(async (name: string) => (name === "work_plans" ? [summary("P1")] : undefined));
  projectPlans.setEnabled(false);
});

describe("project plans", () => {
  it("asks nothing of the engine and lists nothing while work plans are off", async () => {
    await projectPlans.refresh("/work/app");
    expect(invoked).not.toHaveBeenCalled();
    expect(projectPlans.of("/work/app")).toEqual([]);
    expect(projectPlans.all).toEqual([]);
  });

  it("lists a project's plans once on, matching the root as the sidebar does", async () => {
    projectPlans.setEnabled(true);
    await projectPlans.refresh("/work/app");
    expect(invoked).toHaveBeenCalledWith("work_plans", { projectRoot: "/work/app" });
    expect(projectPlans.of("/work/app/").map((p) => p.planId)).toEqual(["P1"]);
    expect(projectPlans.all).toHaveLength(1);
    projectPlans.setEnabled(false);
    expect(projectPlans.of("/work/app")).toEqual([]);
  });
});

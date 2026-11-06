import { invoke } from "@tauri-apps/api/core";
import type { ParallelSet } from "../protocol/ParallelSet";
import type { PlanSummary } from "../protocol/PlanSummary";
import type { TaskPacket } from "../protocol/TaskPacket";
import type { WhyStep } from "../protocol/WhyStep";

/** The project's work plans, newest first. */
export const workPlans = (projectRoot: string) => invoke<PlanSummary[]>("work_plans", { projectRoot });

/** The packet the engine would hand the model for this task now. */
export const workPacket = (sessionId: string, taskId: string) =>
  invoke<TaskPacket>("work_packet", { sessionId, taskId });

/** How this task came to be as it is, oldest step first. */
export const workWhy = (sessionId: string, taskId: string) => invoke<WhyStep[]>("work_why", { sessionId, taskId });

/** Which ready tasks may run in parallel helpers now (a Draft: what approving would allow), and why the others may not. */
export const workParallel = (sessionId: string) => invoke<ParallelSet>("work_parallel", { sessionId });

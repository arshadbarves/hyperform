import { workPacket, workParallel, workWhy } from "../commands";
import type { ParallelSet } from "../protocol/ParallelSet";
import type { TaskPacket } from "../protocol/TaskPacket";
import type { WhyStep } from "../protocol/WhyStep";
import { errorText } from "./toasts";

/** A query answer, or why there is none (no plan attached, or an engine without work plans). */
export type Loaded<T> = { ok: true; value: T } | { ok: false; error: string };

async function load<T>(call: () => Promise<T>): Promise<Loaded<T>> {
  try {
    return { ok: true, value: await call() };
  } catch (e) {
    return { ok: false, error: errorText(e) };
  }
}

/** How a task came to be as it is, loaded when its Why? opens. */
export const taskWhy = (sessionId: string, taskId: string): Promise<Loaded<WhyStep[]>> =>
  load(() => workWhy(sessionId, taskId));

/** What the engine would hand the model for a task now, for the Context tab. */
export const taskPacket = (sessionId: string, taskId: string): Promise<Loaded<TaskPacket>> =>
  load(() => workPacket(sessionId, taskId));

/** Which tasks can run in parallel helpers (a Draft: what approving would allow), for the badges and the Build menu. */
export const parallelSet = (sessionId: string): Promise<Loaded<ParallelSet>> => load(() => workParallel(sessionId));

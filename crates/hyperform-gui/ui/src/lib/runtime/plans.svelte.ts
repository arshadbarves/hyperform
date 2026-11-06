import { workPlans } from "../commands";
import { sameWorkspacePath } from "../domain/paths";
import type { PlanSummary } from "../protocol/PlanSummary";

const SOON_MS = 400;

/**
 * Each project's work plans (`work_plans`), newest first, for the Plan
 * tab's picker, the project home's card and the Inbox. Nothing is asked
 * of the engine while work plans are off, and every list is then empty.
 */
class ProjectPlansStore {
  #byRoot = $state.raw<Record<string, PlanSummary[]>>({});
  enabled = $state(false);
  #timers = new Map<string, ReturnType<typeof setTimeout>>();

  setEnabled(on: boolean): void {
    if (this.enabled === on) return;
    this.enabled = on;
    if (!on) this.#byRoot = {};
  }

  /** The project's plans; empty while off or not loaded yet. */
  of(root: string | null | undefined): PlanSummary[] {
    if (!this.enabled || !root) return [];
    const key = Object.keys(this.#byRoot).find((k) => sameWorkspacePath(k, root));
    return key ? this.#byRoot[key] : [];
  }

  /** Every loaded project's plans. */
  get all(): PlanSummary[] {
    return this.enabled ? Object.values(this.#byRoot).flat() : [];
  }

  async refresh(root: string): Promise<void> {
    if (!this.enabled) return;
    try {
      const plans = await workPlans(root);
      if (this.enabled) this.#byRoot = { ...this.#byRoot, [root]: plans };
    } catch (e) {
      console.error("work_plans failed", e);
    }
  }

  /** Coalesce a burst of plan updates into one listing per project. */
  refreshSoon(root: string, delayMs = SOON_MS): void {
    if (!this.enabled) return;
    const pending = this.#timers.get(root);
    if (pending) clearTimeout(pending);
    this.#timers.set(
      root,
      setTimeout(() => {
        this.#timers.delete(root);
        void this.refresh(root);
      }, delayMs),
    );
  }

  refreshAll(roots: readonly string[]): void {
    for (const root of roots) void this.refresh(root);
  }
}

export const projectPlans = new ProjectPlansStore();

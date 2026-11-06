import { gitSummary, listChangedFiles, listInstructionFiles, type ChangedFile, type RepoSummary } from "../commands";
import { sameWorkspacePath } from "../domain/paths";
import { turnsByProject } from "../domain/sidebarModel";
import type { SessionView } from "../domain/sessionView/types";

/** What the project home shows beyond the summary. */
export interface ProjectDetails {
  files: ChangedFile[];
  /** Whether the project has its own AGENTS.md or CLAUDE.md; null when unknown. */
  instructions: boolean | null;
}

/**
 * Each project's branch and uncommitted-file count, for the sidebar and the
 * project home. Refreshed at start, when the window regains focus, when a
 * project is added, and when a turn ends in a project.
 */
class ProjectsStore {
  #summaries = $state.raw<Record<string, RepoSummary | null>>({});
  #details = $state.raw<Record<string, ProjectDetails>>({});
  #inflight = new Map<string, Promise<void>>();
  #turns: Record<string, number> | null = null;

  summary(root: string | null): RepoSummary | null {
    return root ? (this.#summaries[root] ?? null) : null;
  }

  details(root: string | null): ProjectDetails | null {
    return root ? (this.#details[root] ?? null) : null;
  }

  /** Changed files and instruction files, for the project home. */
  async loadDetails(root: string): Promise<void> {
    const [files, instructions] = await Promise.all([
      listChangedFiles(root).catch(() => [] as ChangedFile[]),
      listInstructionFiles(root)
        .then((list) => list.some((f) => f.scope !== "user"))
        .catch(() => null),
    ]);
    this.#details = { ...this.#details, [root]: { files, instructions } };
  }

  refresh(root: string): Promise<void> {
    const running = this.#inflight.get(root);
    if (running) return running;
    const task = gitSummary(root)
      .then((summary) => this.#set(root, summary))
      .catch(() => this.#set(root, null))
      .finally(() => this.#inflight.delete(root));
    this.#inflight.set(root, task);
    return task;
  }

  async refreshAll(roots: string[]): Promise<void> {
    await Promise.all(roots.map((root) => this.refresh(root)));
  }

  /** Refresh the projects whose chats finished a turn since the last call. */
  noteTurns(views: Record<string, SessionView>, roots: string[]): void {
    const now = turnsByProject(views);
    const before = this.#turns;
    this.#turns = now;
    if (!before) return;
    for (const [root, count] of Object.entries(now)) {
      if (before[root] === count) continue;
      const known = roots.find((r) => sameWorkspacePath(r, root));
      if (known) void this.refresh(known);
    }
  }

  /** Refresh every project whenever the window regains focus; returns the cleanup. */
  watchFocus(roots: () => string[]): () => void {
    const onFocus = () => void this.refreshAll(roots());
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }

  #set(root: string, summary: RepoSummary | null) {
    const prev = this.#summaries[root];
    if (prev !== undefined && JSON.stringify(prev) === JSON.stringify(summary)) return;
    this.#summaries = { ...this.#summaries, [root]: summary };
  }
}

export const projects = new ProjectsStore();

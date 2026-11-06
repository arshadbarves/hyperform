import { invoke } from "@tauri-apps/api/core";

export const listWorkspaces = () => invoke<string[]>("list_workspaces");
export const addWorkspace = (path: string) => invoke<string>("add_workspace", { path });
export const removeWorkspace = (path: string) => invoke("remove_workspace", { path });

export interface ChangedFile {
  path: string;
  /** `added` | `modified` | `deleted` */
  status: string;
  added: number;
  deleted: number;
}

/** A project's repository at a glance; paths in git-scope results are relative to `root`. */
export interface RepoSummary {
  root: string;
  /** Null on a detached HEAD. */
  branch: string | null;
  changed: number;
}

/** Null when the project is not inside a git repository. */
export const gitSummary = (root: string) => invoke<RepoSummary | null>("git_summary", { root });

/** Uncommitted changes of `root`, or of the active project when omitted. */
export const listChangedFiles = (root: string | null = null) => invoke<ChangedFile[]>("list_changed_files", { root });
export const diffForFile = (path: string, root: string | null = null) =>
  invoke<string>("diff_for_file", { path, root });

/** A git worktree of `root` (else the active project) on a new branch, registered as a project. */
export const createWorktree = (name: string, root: string | null = null) =>
  invoke<string>("create_worktree", { name, root });

/** Opens a file or folder inside a project with the system's default app. */
export const openPath = (path: string) => invoke<void>("open_path", { path });
/** Shows a file or folder inside a project in Finder or Explorer. */
export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

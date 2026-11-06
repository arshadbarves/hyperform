import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { tick } from "svelte";
import { createWorktree } from "../commands";
import {
  buildPlan,
  deleteChat,
  errorText,
  newChat,
  openSession,
  pushToast,
  sessionList,
  sessions,
  type BuildRequest,
} from "../runtime";
import { resetShell } from "../shellStore";
import { sameWorkspacePath, workspaceStore, wsBasename } from "../workspaces";
import { composer } from "./composer.svelte";
import { confirmStore } from "./confirm.svelte";
import { ui, type SettingsTab } from "./ui.svelte";

/** Settings opens as a sheet over the app; without a page, it opens where it was left. */
export function openSettings(tab: SettingsTab | null = null, focus: string | null = null): void {
  ui.openSettings(tab, focus);
}

/** Pick a folder and register it as a workspace; returns the new root. */
export async function addWorkspace(): Promise<string | null> {
  try {
    const picked = await openFileDialog({ directory: true, multiple: false, title: "Choose a project folder" });
    if (typeof picked !== "string" || !picked) return null;
    return await addWorkspaceAt(picked);
  } catch (e) {
    console.error(e);
    pushToast("Could not add the project", "warn");
    return null;
  }
}

/** Register a known folder (a drop or a picker result) and make it active. */
export async function addWorkspaceAt(path: string): Promise<string | null> {
  await workspaceStore.add(path);
  return workspaceStore.getSnapshot().active;
}

/** New chat in the active workspace, asking for a folder when there is none. */
export async function startNewChat(): Promise<void> {
  if (!workspaceStore.getSnapshot().roots.length && !(await addWorkspace())) return;
  if (await newChat()) {
    ui.view = "chat";
    composer.clear();
    resetShell();
    composer.focus();
  }
}

/** The active project's home: no chat open, the composer ready for a new one. */
export function goHome(root: string | null = null): void {
  if (root) workspaceStore.setActive(root);
  resetShell();
  sessions.activate(null);
  ui.view = "home";
  void tick().then(() => composer.focus());
}

export function showInbox(): void {
  ui.view = "inbox";
}

export async function openChat(sessionId: string, projectRoot: string): Promise<void> {
  ui.view = "chat";
  if (sessions.activeId === sessionId) return;
  resetShell();
  await openSession(sessionId, projectRoot);
}

/** Open a chat known only by id; its project comes from the live view or the chat list. */
export function openChatById(sessionId: string): Promise<void> {
  const root =
    sessions.view(sessionId)?.info?.projectRoot ??
    sessionList.summaries.find((s) => s.sessionId === sessionId)?.projectRoot ??
    "";
  return openChat(sessionId, root);
}

export async function removeChat(sessionId: string, title?: string): Promise<void> {
  const ok = await confirmStore.ask({
    title: title ? `Delete “${title}”?` : "Delete this chat?",
    description: "The conversation is removed from this computer; your files and the project's code checkpoints stay. This cannot be undone.",
    confirmLabel: "Delete chat",
    tone: "danger",
  });
  if (ok) await deleteChat(sessionId);
}

export async function removeWorkspace(root: string): Promise<void> {
  const ok = await confirmStore.ask({
    title: `Remove ${wsBasename(root)} from HyperForm?`,
    description: "Its chats are deleted from this computer. The folder and your files stay untouched.",
    confirmLabel: "Remove project",
    tone: "danger",
  });
  if (!ok) return;
  const activeRoot = sessions.active?.info?.projectRoot;
  try {
    await workspaceStore.remove(root);
  } catch (e) {
    console.error(e);
    pushToast("Could not remove the project", "warn");
    return;
  }
  if (sameWorkspacePath(activeRoot, root)) sessions.activate(null);
  void sessionList.refresh();
}

/** Build from the Plan tab; a new chat that builds then shows like any new chat. */
export async function startBuild(req: BuildRequest): Promise<boolean> {
  const id = await buildPlan(req);
  if (id && req.target === "newChat") {
    ui.view = "chat";
    composer.clear();
    resetShell();
  }
  return id !== null;
}

/** New chat in a given project. */
export async function startNewChatIn(root: string): Promise<void> {
  workspaceStore.setActive(root);
  await startNewChat();
}

/** A worktree of `from` (else the active project) on its own branch, with a new chat in it. */
export async function createWorktreeAndStart(name: string, from: string | null = null): Promise<void> {
  try {
    const root = await createWorktree(name, from ?? workspaceStore.getSnapshot().active);
    await workspaceStore.load();
    workspaceStore.setActive(root);
    await startNewChat();
  } catch (e) {
    console.error(e);
    pushToast(errorText(e), "warn");
  }
}

import type { SessionView } from "./sessionView/types";

/** The page the main stage is asked to show. */
export type StageView = "home" | "chat" | "inbox";

export interface ActiveChat {
  /** Anything the transcript would draw exists. */
  hasContent: boolean;
  /** Opened, snapshot not arrived yet. */
  hydrating: boolean;
}

/** What the stage actually shows: an empty new chat is still the project home. */
export function stageFor(view: StageView, active: ActiveChat | null): StageView {
  if (view === "inbox") return "inbox";
  if (view === "home" || !active) return "home";
  return active.hasContent || active.hydrating ? "chat" : "home";
}

type ContentView = Pick<
  SessionView,
  "messages" | "status" | "streaming" | "outputs" | "errors" | "trustRequest" | "approvals" | "questions" | "plans"
>;

/** True once a chat has something to show: a message, live work, an output, or a question for the user. */
export function hasChatContent(view: ContentView | null): boolean {
  if (!view) return false;
  return (
    view.messages.length > 0 ||
    view.status !== "idle" ||
    Object.keys(view.streaming).length > 0 ||
    view.outputs.some((o) => o.name !== "shell") ||
    view.errors.length > 0 ||
    view.trustRequest !== null ||
    Object.keys(view.approvals).length > 0 ||
    Object.keys(view.questions).length > 0 ||
    Object.keys(view.plans).length > 0
  );
}

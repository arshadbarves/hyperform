import { longChatSnapshot } from "../domain/timeline/longChat";
import { ui } from "../stores/ui.svelte";
import { workspaceStore } from "../workspaces";
import { sessions } from "./sessions.svelte";

type DevHooks = { longChat?: (turns?: number) => string };

/**
 * Dev builds only (main.ts loads this behind `import.meta.env.DEV`): run
 * `__zengine.longChat(1000)` in the webview console to open a finished,
 * local-only chat of that many turns for performance checks. It never
 * reaches the engine; the chat is gone on reload.
 */
export function installDevChat(): void {
  const target = globalThis as typeof globalThis & { __zengine?: DevHooks };
  target.__zengine = {
    ...target.__zengine,
    longChat(turns = 1000) {
      const id = `dev-long-chat-${turns}-${Date.now()}`;
      const workspaces = workspaceStore.getSnapshot();
      const root = workspaces.active ?? workspaces.roots[0] ?? "/tmp/long-chat";
      const snapshot = longChatSnapshot(turns, id, root);
      sessions.apply({ sessionId: id, seq: 0, event: { type: "snapshot", snapshot } });
      sessions.activate(id);
      ui.view = "chat";
      return id;
    },
  };
}

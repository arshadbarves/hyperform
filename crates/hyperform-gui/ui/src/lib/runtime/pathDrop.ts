import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * Files and folders dragged onto the window from the system, with their
 * paths (the browser's own drop events carry no paths). Returns the unlisten.
 */
export function onPathDrop(handlers: { hover?: (over: boolean) => void; drop: (paths: string[]) => void }): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "over") handlers.hover?.(true);
    else if (payload.type === "leave") handlers.hover?.(false);
    else if (payload.type === "drop") {
      handlers.hover?.(false);
      handlers.drop(payload.paths);
    }
  });
}

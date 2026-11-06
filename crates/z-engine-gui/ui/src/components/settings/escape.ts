import type { Attachment } from "svelte/attachments";

/** Escape inside the element cancels it without also closing the Settings page. */
export function cancelOnEscape(cancel: () => void): Attachment<HTMLElement> {
  return (node) => {
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      cancel();
    };
    node.addEventListener("keydown", onKeydown);
    return () => node.removeEventListener("keydown", onKeydown);
  };
}

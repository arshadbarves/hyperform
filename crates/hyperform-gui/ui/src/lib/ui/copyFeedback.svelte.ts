import { onDestroy } from "svelte";

/** Clipboard copy with a short "Copied" confirmation. The revert timer is
 *  cancelled when the owning component unmounts, so a copy made just before a
 *  turn re-renders cannot write to a destroyed component. */
export function copyFeedback(resetMs = 1200) {
  let copied = $state(false);
  let timer: number | undefined;

  function clear() {
    if (timer === undefined) return;
    window.clearTimeout(timer);
    timer = undefined;
  }

  onDestroy(clear);

  /** Returns false when the clipboard rejected the write, so callers can
   *  surface it in whichever way suits their surface. */
  async function copy(text: string): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      return false;
    }
    copied = true;
    clear();
    timer = window.setTimeout(() => {
      copied = false;
      timer = undefined;
    }, resetMs);
    return true;
  }

  return {
    get copied() {
      return copied;
    },
    copy,
  };
}

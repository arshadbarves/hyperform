import { on } from "svelte/events";
import type { Point } from "$lib/domain/pet/physics";
import { TYPING_MS } from "$lib/domain/pet/pose";
import { perches } from "$lib/ui/perch.svelte";

/** A pointer that stopped this long ago no longer holds the pet's eyes. */
const POINTER_MS = 2500;

const MIRRORED = [
  "boxSizing",
  "width",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontStyle",
  "letterSpacing",
  "lineHeight",
  "textTransform",
  "wordSpacing",
  "textIndent",
  "tabSize",
] as const;

/** Where a textarea's text cursor is on screen, measured on a hidden copy of its text. */
function caretPoint(el: HTMLTextAreaElement): Point {
  const style = getComputedStyle(el);
  const mirror = document.createElement("div");
  for (const key of MIRRORED) mirror.style[key] = style[key];
  Object.assign(mirror.style, {
    position: "fixed",
    top: "0",
    left: "-9999px",
    visibility: "hidden",
    whiteSpace: "pre-wrap",
    overflowWrap: "break-word",
  });
  mirror.textContent = el.value.slice(0, el.selectionEnd ?? el.value.length);
  const mark = document.createElement("span");
  mark.textContent = "\u200b";
  mirror.append(mark);
  document.body.append(mirror);
  const box = el.getBoundingClientRect();
  const x = box.left + mark.offsetLeft - el.scrollLeft;
  const y = box.top + mark.offsetTop - el.scrollTop + mark.offsetHeight / 2;
  mirror.remove();
  return { x: Math.min(box.right, Math.max(box.left, x)), y: Math.min(box.bottom, Math.max(box.top, y)) };
}

/**
 * What the pet's eyes follow: the composer's text cursor while you type,
 * otherwise (while `pointer()` holds) the pointer while it moves. Null
 * when there is nothing to look at, so the pose's own gaze shows. Call
 * during component init.
 */
export function useLookTarget(options: { pointer: () => boolean; enabled: () => boolean }) {
  let caret = $state<Point | null>(null);
  let pointer = $state<Point | null>(null);

  $effect(() => {
    if (!options.enabled()) return;
    const follow = options.pointer();
    let frame = 0;
    let caretFrame = 0;
    let caretTimer = 0;
    let pointerTimer = 0;
    let latest: Point | null = null;
    const measure = () => {
      frame = 0;
      if (latest) pointer = latest;
      latest = null;
    };
    const readCaret = () => {
      caretFrame = 0;
      const el = document.activeElement;
      if (!(el instanceof HTMLTextAreaElement) || !perches.node("composer")?.contains(el)) return;
      caret = caretPoint(el);
      window.clearTimeout(caretTimer);
      caretTimer = window.setTimeout(() => (caret = null), TYPING_MS);
    };
    const onEdit = () => {
      caretFrame ||= requestAnimationFrame(readCaret);
    };
    const onLeave = () => (caret = null);
    const onMove = (e: PointerEvent) => {
      latest = { x: e.clientX, y: e.clientY };
      frame ||= requestAnimationFrame(measure);
      window.clearTimeout(pointerTimer);
      pointerTimer = window.setTimeout(() => (pointer = null), POINTER_MS);
    };
    document.addEventListener("input", onEdit, true);
    document.addEventListener("selectionchange", onEdit);
    // Removing a focused node fires focusout mid-update, where only a
    // handler attached through Svelte may write state.
    const offLeave = on(document, "focusout", onLeave, { capture: true });
    if (follow) window.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      cancelAnimationFrame(caretFrame);
      window.clearTimeout(caretTimer);
      window.clearTimeout(pointerTimer);
      document.removeEventListener("input", onEdit, true);
      document.removeEventListener("selectionchange", onEdit);
      offLeave();
      window.removeEventListener("pointermove", onMove);
      caret = null;
      pointer = null;
    };
  });

  return {
    get point(): Point | null {
      return caret ?? pointer;
    },
  };
}

import { inbox } from "./inbox.svelte";
import { sessions } from "./sessions.svelte";

export interface ToastAction {
  label: string;
  onclick?: () => void;
  variant?: "primary" | "secondary";
}

export interface Toast {
  id: number;
  text: string;
  title?: string;
  tag?: string;
  tone: "info" | "ok" | "warn" | "error";
  actions?: ToastAction[];
  onDismiss?: () => void;
}

export type ToastInput = string | (Partial<Toast> & { text: string });

type Listener = () => void;

const listeners = new Set<Listener>();
let toasts: Toast[] = [];
let nextId = 1;

function emit() {
  for (const l of listeners) l();
}

/** `{ subscribe, getSnapshot }` so the title status line binds it with `bindStore`. */
export const toastStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): Toast[] {
    return toasts;
  },
};

export function dismissToast(id: number) {
  toasts.find((t) => t.id === id)?.onDismiss?.();
  toasts = toasts.filter((t) => t.id !== id);
  emit();
}

function splitTitle(text: string): { title?: string; tag?: string } {
  for (const sep of [" · ", " ─ "]) {
    if (!text.includes(sep)) continue;
    const [title, tag] = text.split(sep);
    return { title: title?.trim(), tag: tag?.trim() };
  }
  return {};
}

export function pushToast(input: ToastInput, toneArg: Toast["tone"] = "info"): number {
  const id = nextId++;
  const toast: Toast =
    typeof input === "string"
      ? { id, text: input, tone: toneArg, ...splitTitle(input) }
      : { ...input, id, tone: input.tone ?? toneArg };
  toasts = [...toasts.slice(-3), toast];
  emit();
  inbox.record({
    key: `t${id}`,
    tone: toast.tone,
    title: toast.title ?? toast.text,
    text: toast.text,
    sessionId: sessions.activeId,
    at: Date.now(),
  });
  const life = toast.actions?.length ? 9000 : toast.tone === "warn" || toast.tone === "error" ? 6000 : 4500;
  setTimeout(() => {
    toasts = toasts.filter((t) => t.id !== id);
    emit();
  }, life);
  return id;
}

export function errorText(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  return raw.replace(/^Error:\s*/, "");
}

const PREVIEW_CHARS = 48;

/** One-line label for a queued steering message so the composer queue row never wraps. */
export function queuePreview(item: string): string {
  const text = item.replace(/\s*[\r\n]+\s*/g, " ").trim();
  if (!text) return "Empty follow-up";
  return text.length > PREVIEW_CHARS ? `${text.slice(0, PREVIEW_CHARS - 1)}…` : text;
}

/** Hover text for a queued message: the full prompt, never blank. */
export function queueTitle(item: string): string {
  return item.trim() || queuePreview(item);
}

export function replaceQueued(queue: string[], index: number, text: string): string[] {
  const clean = text.trim();
  if (!clean) return removeQueued(queue, index);
  return queue.map((item, i) => (i === index ? clean : item));
}

export function removeQueued(queue: string[], index: number): string[] {
  return queue.filter((_, i) => i !== index);
}

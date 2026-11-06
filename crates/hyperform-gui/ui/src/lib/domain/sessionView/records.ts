import type { Message } from "../../protocol/Message";

/** Immutable record/array helpers shared by the reducer steps. */

export function put<T>(record: Record<string, T>, key: string, value: T): Record<string, T> {
  return { ...record, [key]: value };
}

export function drop<T>(record: Record<string, T>, key: string): Record<string, T> {
  if (!(key in record)) return record;
  const next = { ...record };
  delete next[key];
  return next;
}

export function filterRecord<T>(
  record: Record<string, T>,
  keep: (value: T, key: string) => boolean,
): Record<string, T> {
  const next: Record<string, T> = {};
  let changed = false;
  for (const [key, value] of Object.entries(record)) {
    if (keep(value, key)) next[key] = value;
    else changed = true;
  }
  return changed ? next : record;
}

export function mapRecord<T>(
  record: Record<string, T>,
  map: (value: T, key: string) => T,
): Record<string, T> {
  let changed = false;
  const next: Record<string, T> = {};
  for (const [key, value] of Object.entries(record)) {
    const mapped = map(value, key);
    if (mapped !== value) changed = true;
    next[key] = mapped;
  }
  return changed ? next : record;
}

export function byKey<T>(items: T[], key: (item: T) => string): Record<string, T> {
  const out: Record<string, T> = {};
  for (const item of items) out[key(item)] = item;
  return out;
}

/** Replace the item with the same key in place, or append it. */
export function upsertBy<T>(items: T[], item: T, key: (item: T) => string): T[] {
  const id = key(item);
  const index = items.findIndex((existing) => key(existing) === id);
  if (index < 0) return [...items, item];
  const next = items.slice();
  next[index] = item;
  return next;
}

export function upsertMessage(messages: Message[], message: Message): Message[] {
  return upsertBy(messages, message, (m) => m.id);
}

export function lastMessageId(messages: Message[]): string | null {
  return messages.length > 0 ? messages[messages.length - 1].id : null;
}

/** Keep the newest `max` entries. */
export function capped<T>(items: T[], item: T, max: number): T[] {
  const next = [...items, item];
  return next.length > max ? next.slice(next.length - max) : next;
}

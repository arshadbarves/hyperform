/** Immutable edits of an ordered settings list (hooks, checks). */

export function appendItem<T>(list: readonly T[], item: T): T[] {
  return [...list, item];
}

export function replaceItem<T>(list: readonly T[], index: number, item: T): T[] {
  return index >= 0 && index < list.length ? list.map((old, i) => (i === index ? item : old)) : [...list];
}

export function removeItem<T>(list: readonly T[], index: number): T[] {
  return list.filter((_, i) => i !== index);
}

/** Moves one item by `delta` places; moves past either end do nothing. */
export function moveItem<T>(list: readonly T[], index: number, delta: number): T[] {
  const target = index + delta;
  if (index < 0 || index >= list.length || target < 0 || target >= list.length) return [...list];
  const next = [...list];
  const [item] = next.splice(index, 1);
  next.splice(target, 0, item);
  return next;
}

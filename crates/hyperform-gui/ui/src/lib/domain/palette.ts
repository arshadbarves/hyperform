/** Command palette ranking: what you typed against each row's name and words. */

export interface Rankable {
  label: string;
  keywords: string;
  group?: string;
}

/** Lower is better; null when the row does not match at all. */
export function paletteScore(query: string, item: Rankable): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const label = item.label.toLowerCase();
  if (label.startsWith(q)) return 0;
  if (label.split(/[\s/·:-]+/).some((word) => word.startsWith(q))) return 1;
  if (label.includes(q)) return 2;
  if (`${item.keywords} ${item.group ?? ""}`.toLowerCase().includes(q)) return 3;
  return subsequence(q, label);
}

/**
 * Letters of the name typed in order, starting at the start of a word ("nc"
 * finds "New chat"); scored by how spread out they are, after every real match.
 */
function subsequence(q: string, hay: string): number | null {
  const start = [...hay.matchAll(/(^|[\s/·:-])(\S)/g)].find((m) => m[2] === q[0]);
  if (!start || start.index === undefined) return null;
  const first = start.index + start[1].length;
  let prev = first;
  for (const ch of q.slice(1)) {
    const at = hay.indexOf(ch, prev + 1);
    if (at === -1) return null;
    prev = at;
  }
  const span = prev - first;
  return span > q.length * 2 + 2 ? null : 4 + span / 100;
}

export function rankPalette<T extends Rankable>(items: readonly T[], query: string): T[] {
  return items
    .map((item, order) => ({ item, order, score: paletteScore(query, item) }))
    .filter((r): r is { item: T; order: number; score: number } => r.score !== null)
    .sort((a, b) => a.score - b.score || a.order - b.order)
    .map((r) => r.item);
}

export interface PaletteGroup<T> {
  name: string | undefined;
  items: { item: T; index: number }[];
}

export function groupPalette<T extends Rankable>(items: readonly T[]): PaletteGroup<T>[] {
  const out: PaletteGroup<T>[] = [];
  items.forEach((item, index) => {
    const group = out.find((g) => g.name === item.group);
    if (group) group.items.push({ item, index });
    else out.push({ name: item.group, items: [{ item, index }] });
  });
  return out;
}

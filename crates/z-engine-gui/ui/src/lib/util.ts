const DAY = 86_400_000;

export function relTime(ms: number, now = Date.now()): string {
  const d = now - ms;
  if (d < 60_000) return "now";
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`;
  if (d < DAY) return `${Math.floor(d / 3_600_000)}h`;
  return `${Math.floor(d / DAY)}d`;
}

export function shortModel(id: string): string {
  const slash = id.lastIndexOf("/");
  return slash >= 0 ? id.slice(slash + 1) : id;
}

/** `1234` → `"1.2k"`; below 1000 stays plain. */
export function fmtTokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(2)}M`;
}

export function fmtCost(usd: number | null): string {
  if (usd == null) return "–";
  if (usd === 0) return "$0.00";
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

/** `850ms`, `4.2s`, `3m 05s`, `1h 12m`. */
export function fmtDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return "";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const totalSeconds = Math.floor(ms / 1000);
  if (totalSeconds < 3600) {
    const m = Math.floor(totalSeconds / 60);
    const s = totalSeconds % 60;
    return `${m}m ${String(s).padStart(2, "0")}s`;
  }
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  return `${h}h ${m}m`;
}

/** Elapsed since `startedAt`, or the recorded duration when finished. */
export function elapsed(startedAt: number | null, finishedAt: number | null, now: number): string {
  if (startedAt === null) return "";
  return fmtDuration((finishedAt ?? now) - startedAt);
}

export function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

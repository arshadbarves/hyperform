import type { GitDiffRow } from "../diffParse";

/** A stretch of unchanged lines shown as one "N unchanged lines" row. */
export interface FoldRow {
  kind: "fold";
  /** Index of the first hidden row in the unfolded rows. */
  from: number;
  count: number;
}

export type DiffDisplayRow = GitDiffRow | FoldRow;

const KEEP = 3;
const MIN_FOLD = 8;

/**
 * Hide long unchanged stretches, keeping `KEEP` lines next to each change.
 * `open` holds the `from` of folds the reader expanded.
 */
export function foldContext(rows: GitDiffRow[], open: ReadonlySet<number> = new Set()): DiffDisplayRow[] {
  const out: DiffDisplayRow[] = [];
  let i = 0;
  while (i < rows.length) {
    if (rows[i]?.kind !== "ctx") {
      out.push(rows[i] as GitDiffRow);
      i++;
      continue;
    }
    let end = i;
    while (end < rows.length && rows[end]?.kind === "ctx") end++;
    const head = i === 0 ? 0 : KEEP;
    const tail = end === rows.length ? 0 : KEEP;
    const hidden = end - i - head - tail;
    const from = i + head;
    if (hidden >= MIN_FOLD && !open.has(from)) {
      out.push(...rows.slice(i, from));
      out.push({ kind: "fold", from, count: hidden });
      out.push(...rows.slice(from + hidden, end));
    } else {
      out.push(...rows.slice(i, end));
    }
    i = end;
  }
  return out;
}

export type SplitRow =
  | { kind: "pair"; key: string; left: GitDiffRow | null; right: GitDiffRow | null }
  | { kind: "hunk"; key: string; row: GitDiffRow }
  | { kind: "fold"; key: string; row: FoldRow };

/** Side-by-side rows: old on the left, new on the right, replacements aligned. */
export function splitRows(rows: DiffDisplayRow[]): SplitRow[] {
  const out: SplitRow[] = [];
  let dels: GitDiffRow[] = [];
  let adds: GitDiffRow[] = [];
  const flush = () => {
    for (let k = 0; k < Math.max(dels.length, adds.length); k++) {
      out.push({ kind: "pair", key: `p${out.length}`, left: dels[k] ?? null, right: adds[k] ?? null });
    }
    dels = [];
    adds = [];
  };
  for (const row of rows) {
    if (row.kind === "fold") {
      flush();
      out.push({ kind: "fold", key: `f${row.from}`, row });
    } else if (row.kind === "del") {
      if (adds.length) flush();
      dels.push(row);
    } else if (row.kind === "add") {
      adds.push(row);
    } else {
      flush();
      if (row.kind === "ctx") out.push({ kind: "pair", key: `p${out.length}`, left: row, right: row });
      else out.push({ kind: "hunk", key: `h${out.length}`, row });
    }
  }
  flush();
  return out;
}

const LANGS: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  py: "python",
  json: "json",
  css: "css",
  html: "xml",
  svelte: "xml",
  vue: "xml",
  xml: "xml",
  svg: "xml",
  yml: "yaml",
  yaml: "yaml",
  md: "markdown",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  toml: "ini",
  ini: "ini",
};

/** The highlighter language for a file, from its extension. */
export function langForPath(path: string): string | null {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  return path.includes(".") ? (LANGS[ext] ?? null) : null;
}

export interface FileSummary {
  files: number;
  created: number;
  updated: number;
  deleted: number;
  added: number;
  removed: number;
}

export function summarizeFiles(
  files: readonly { status: string; added?: number | null; deleted?: number | null }[],
): FileSummary {
  const s: FileSummary = { files: files.length, created: 0, updated: 0, deleted: 0, added: 0, removed: 0 };
  for (const f of files) {
    if (f.status === "added") s.created++;
    else if (f.status === "deleted") s.deleted++;
    else s.updated++;
    s.added += f.added ?? 0;
    s.removed += f.deleted ?? 0;
  }
  return s;
}

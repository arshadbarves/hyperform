export type DiffLineKind = "add" | "del" | "hunk" | "meta" | "ctx";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

export type GitDiffRowKind = "add" | "del" | "ctx" | "hunk";

export interface GitDiffRow {
  kind: GitDiffRowKind;
  oldNo: number | null;
  newNo: number | null;
  text: string;
}

export interface GitDiff {
  path: string | null;
  added: number;
  deleted: number;
  rows: GitDiffRow[];
}

export function looksLikeDiff(text: string): boolean {
  return /(^|\n)(@@ |--- |\+\+\+ |diff --git )/.test(text);
}

function kindOf(line: string): DiffLineKind {
  if (
    line.startsWith("diff ") ||
    line.startsWith("index ") ||
    line.startsWith("---") ||
    line.startsWith("+++") ||
    line.startsWith("new file") ||
    line.startsWith("deleted file") ||
    line.startsWith("similarity ") ||
    line.startsWith("rename ")
  ) {
    return "meta";
  }
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "del";
  return "ctx";
}

/** Split a unified diff into typed lines. `---` / `+++` are headers, not
 * deletions/additions — check those before the single-character markers. */
export function parseUnifiedDiff(text: string): DiffLine[] {
  const body = text.replace(/\n$/, "");
  if (!body) return [];
  return body.split("\n").map((line) => ({ kind: kindOf(line), text: line }));
}

/** `+++ b/src/lib.rs` → `src/lib.rs`; `+++ b//Users/x` → `/Users/x`. */
export function cleanDiffPath(raw: string): string | null {
  let p = raw.trim();
  if (!p || p === "/dev/null") return null;
  p = p.replace(/^[ab]\//, "");
  p = p.replace(/^\/\//, "/");
  return p || null;
}

const HUNK_RE = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

/** GitHub-style model: file path, +/- counts, numbered rows without
 * `---` / `+++` / `@@` dump lines. Later hunks become separators. */
export function parseGitDiff(text: string): GitDiff {
  const rows: GitDiffRow[] = [];
  let oldPath: string | null = null;
  let newPath: string | null = null;
  let oldLine = 0;
  let newLine = 0;
  let inHunk = false;
  let hunkCount = 0;
  let added = 0;
  let deleted = 0;

  const body = text.replace(/\n$/, "");
  if (!body) return { path: null, added: 0, deleted: 0, rows };

  let seenFileHeader = false;

  for (const line of body.split("\n")) {
    if (
      line.startsWith("diff ") ||
      line.startsWith("index ") ||
      line.startsWith("new file") ||
      line.startsWith("deleted file") ||
      line.startsWith("similarity ") ||
      line.startsWith("rename ")
    ) {
      continue;
    }
    if (line.startsWith("--- ")) {
      oldPath = cleanDiffPath(line.slice(4));
      seenFileHeader = true;
      continue;
    }
    if (line.startsWith("+++ ")) {
      newPath = cleanDiffPath(line.slice(4));
      seenFileHeader = true;
      continue;
    }
    const hunk = HUNK_RE.exec(line);
    if (hunk) {
      const oldStart = Number(hunk[1]);
      const newStart = Number(hunk[2]);
      if (hunkCount > 0) {
        rows.push({
          kind: "hunk",
          oldNo: oldStart || null,
          newNo: newStart || null,
          text: "",
        });
      }
      hunkCount += 1;
      oldLine = oldStart;
      newLine = newStart;
      inHunk = true;
      continue;
    }
    // Some previews omit @@ (hand-rolled /dev/null diffs). Treat the body
    // as one synthetic hunk so created-file content is still visible.
    if (
      !inHunk &&
      seenFileHeader &&
      (line.startsWith("+") || line.startsWith("-") || line.startsWith(" "))
    ) {
      inHunk = true;
      hunkCount = 1;
      if (line.startsWith("+")) {
        oldLine = 0;
        newLine = 1;
      } else if (line.startsWith("-")) {
        oldLine = 1;
        newLine = 0;
      } else {
        oldLine = 1;
        newLine = 1;
      }
    }
    if (line.startsWith("\\") || !inHunk) continue;
    if (line.startsWith("+")) {
      rows.push({
        kind: "add",
        oldNo: null,
        newNo: newLine > 0 ? newLine : null,
        text: line.slice(1),
      });
      if (newLine > 0) newLine += 1;
      added += 1;
      continue;
    }
    if (line.startsWith("-")) {
      rows.push({
        kind: "del",
        oldNo: oldLine > 0 ? oldLine : null,
        newNo: null,
        text: line.slice(1),
      });
      if (oldLine > 0) oldLine += 1;
      deleted += 1;
      continue;
    }
    const src = line.startsWith(" ") ? line.slice(1) : line;
    rows.push({
      kind: "ctx",
      oldNo: oldLine > 0 ? oldLine : null,
      newNo: newLine > 0 ? newLine : null,
      text: src,
    });
    if (oldLine > 0) oldLine += 1;
    if (newLine > 0) newLine += 1;
  }

  return { path: newPath ?? oldPath, added, deleted, rows };
}

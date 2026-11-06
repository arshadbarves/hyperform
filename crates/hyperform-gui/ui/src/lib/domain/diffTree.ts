/** Pure helpers: workspace-relative paths → collapsible folder tree. */

export type DiffTreeStatus = "added" | "modified" | "deleted" | string;

export type DiffTreeFile = {
  path: string;
  status: DiffTreeStatus;
  added?: number;
  deleted?: number;
};

export type DiffTreeNode =
  | { kind: "dir"; name: string; path: string; children: DiffTreeNode[] }
  | {
      kind: "file";
      name: string;
      path: string;
      status: DiffTreeStatus;
      added: number;
      deleted: number;
    };

/** Build a sorted folder tree from changed-file paths. */
export function buildDiffTree(files: DiffTreeFile[]): DiffTreeNode[] {
  type MutableDir = {
    kind: "dir";
    name: string;
    path: string;
    dirs: Map<string, MutableDir>;
    files: DiffTreeNode[];
  };

  const root: MutableDir = {
    kind: "dir",
    name: "",
    path: "",
    dirs: new Map(),
    files: [],
  };

  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path));
  for (const f of sorted) {
    const parts = f.path.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    let cur = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const name = parts[i]!;
      const dirPath = parts.slice(0, i + 1).join("/");
      let next = cur.dirs.get(name);
      if (!next) {
        next = { kind: "dir", name, path: dirPath, dirs: new Map(), files: [] };
        cur.dirs.set(name, next);
      }
      cur = next;
    }
    const name = parts[parts.length - 1]!;
    cur.files.push({
      kind: "file",
      name,
      path: f.path,
      status: f.status,
      added: f.added ?? 0,
      deleted: f.deleted ?? 0,
    });
  }

  // A folder holding only one folder shows as one row: `crates/app/src`.
  function compact(d: MutableDir): DiffTreeNode {
    let name = d.name;
    let cur = d;
    while (cur.files.length === 0 && cur.dirs.size === 1) {
      const only = [...cur.dirs.values()][0]!;
      name = `${name}/${only.name}`;
      cur = only;
    }
    return { kind: "dir", name, path: cur.path, children: freeze(cur) };
  }

  function freeze(dir: MutableDir): DiffTreeNode[] {
    const dirs = [...dir.dirs.values()].sort((a, b) => a.name.localeCompare(b.name)).map(compact);
    const files = [...dir.files].sort((a, b) => a.name.localeCompare(b.name));
    return [...dirs, ...files];
  }

  return freeze(root);
}

/** Leaf paths in tree display order (dirs before files, alpha). */
export function flattenDiffTree(nodes: DiffTreeNode[]): string[] {
  const out: string[] = [];
  const walk = (list: DiffTreeNode[]) => {
    for (const n of list) {
      if (n.kind === "file") out.push(n.path);
      else walk(n.children);
    }
  };
  walk(nodes);
  return out;
}

/** Keep folders that still contain a matching leaf after filter. */
export function filterDiffTree(nodes: DiffTreeNode[], query: string): DiffTreeNode[] {
  const q = query.trim().toLowerCase();
  if (!q) return nodes;

  const filter = (list: DiffTreeNode[]): DiffTreeNode[] => {
    const out: DiffTreeNode[] = [];
    for (const n of list) {
      if (n.kind === "file") {
        if (n.path.toLowerCase().includes(q)) out.push(n);
        continue;
      }
      const children = filter(n.children);
      if (children.length > 0) {
        out.push({ ...n, children });
      }
    }
    return out;
  };
  return filter(nodes);
}

/** Folder paths that should be open so `selectedPath` is visible. */
export function expandAncestors(selectedPath: string | null): Set<string> {
  const open = new Set<string>();
  if (!selectedPath) return open;
  const parts = selectedPath.split("/").filter(Boolean);
  for (let i = 1; i < parts.length; i++) {
    open.add(parts.slice(0, i).join("/"));
  }
  return open;
}

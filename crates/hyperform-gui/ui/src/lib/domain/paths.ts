/** Last segment of a folder path: the name a project goes by. */
export function wsBasename(root: string): string {
  const parts = root.replace(/[/\\]+$/, "").split(/[/\\]/);
  return parts[parts.length - 1] || root;
}

/** True when two workspace roots refer to the same folder. */
export function sameWorkspacePath(a: string | null | undefined, b: string | null | undefined): boolean {
  if (!a || !b) return false;
  return normalizeWsPath(a) === normalizeWsPath(b);
}

function normalizeWsPath(p: string): string {
  return p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

/** `path` as an absolute path: kept when it already is one, else joined onto `root`. */
export function absolutePath(root: string, path: string): string {
  return /^([/\\]|[A-Za-z]:[/\\])/.test(path) ? path : joinPath(root, path);
}

/** `relative` joined onto `root` with the root's own separator. */
export function joinPath(root: string, relative: string): string {
  const sep = root.includes("\\") && !root.includes("/") ? "\\" : "/";
  return `${root.replace(/[/\\]+$/, "")}${sep}${relative.replace(/^[/\\]+/, "")}`;
}

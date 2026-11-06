export type RememberScope = "project" | "local" | "user";

export interface RememberTarget {
  scope: RememberScope;
  label: string;
  file: string;
}

/** Where a `#` memory goes; the engine resolves the actual paths. */
export const REMEMBER_TARGETS: RememberTarget[] = [
  { scope: "project", label: "Project memory", file: "AGENTS.md" },
  { scope: "local", label: "Personal project memory", file: "AGENTS.local.md" },
  { scope: "user", label: "User memory", file: "All projects" },
];

/** `runCommand { name: "remember", args }` payload. */
export function rememberArgs(scope: RememberScope, text: string): string {
  return `${scope} ${text.trim()}`;
}

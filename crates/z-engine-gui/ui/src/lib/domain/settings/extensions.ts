import type { ExtensionKind } from "../../commands/settings";
import type { ExtensionScope } from "../../protocol/config/ExtensionScope";
import type { ExtensionSource } from "../../protocol/config/ExtensionSource";
import type { Extensions } from "../../protocol/config/Extensions";

export type { ExtensionKind };

export interface ExtensionKindMeta {
  kind: ExtensionKind;
  label: string;
  singular: string;
  description: string;
}

export const EXTENSION_KINDS: readonly ExtensionKindMeta[] = [
  { kind: "agents", label: "Agents", singular: "agent", description: "Subagents the model can delegate to, with their own prompt, tools and model." },
  { kind: "commands", label: "Commands", singular: "command", description: "Slash commands: prompt templates run with /name." },
  { kind: "skills", label: "Skills", singular: "skill", description: "Instructions and bundled files the model loads when a task needs them." },
  { kind: "rules", label: "Rules", singular: "rule", description: "Guidance attached to requests always, or when matching files are involved." },
  { kind: "output-styles", label: "Output styles", singular: "output style", description: "Replacement response styles, chosen under Appearance." },
];

export const EXTENSION_SCOPE_LABELS: Record<ExtensionScope, string> = {
  claudeUser: "~/.claude",
  user: "User",
  claudeProject: ".claude",
  project: "Project",
};

/** One listed definition, whichever kind it is. */
export interface ExtensionEntry {
  kind: ExtensionKind;
  name: string;
  description: string;
  source: ExtensionSource;
  /** The `name` argument of `write_extension_file` that rewrites this file. */
  fileName: string | null;
}

/** `.claude` folders are read for compatibility and never written. */
export function isNativeScope(scope: ExtensionScope): boolean {
  return scope === "user" || scope === "project";
}

export function extensionEntries(extensions: Extensions, kind: ExtensionKind): ExtensionEntry[] {
  const defs = {
    agents: extensions.agents,
    commands: extensions.commands,
    skills: extensions.skills,
    rules: extensions.rules,
    "output-styles": extensions.outputStyles,
  }[kind];
  return defs.map((def) => ({
    kind,
    name: def.name,
    description: def.description,
    source: def.source,
    fileName: extensionFileName(kind, def.source.path),
  }));
}

/** The file's name under its kind folder: `sub/name.md` is `sub:name`; a skill is its folder. */
export function extensionFileName(kind: ExtensionKind, path: string): string | null {
  const parts = path.split(/[\\/]+/);
  const at = parts.lastIndexOf(kind);
  if (at < 0 || at === parts.length - 1) return null;
  const rest = parts.slice(at + 1);
  if (kind === "skills") return rest.length === 2 && rest[1] === "SKILL.md" ? rest[0] : null;
  const last = rest[rest.length - 1];
  if (!last.endsWith(".md")) return null;
  rest[rest.length - 1] = last.slice(0, -3);
  return rest.join(":");
}

const SEGMENT = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

/** Commands and rules may nest folders with `:`; other kinds are one name. */
export function extensionNameError(kind: ExtensionKind, name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Enter a name.";
  if (trimmed.length > 64) return "Keep the name under 64 characters.";
  const nested = kind === "commands" || kind === "rules";
  const segments = nested ? trimmed.split(":") : [trimmed];
  if (!segments.every((segment) => SEGMENT.test(segment))) {
    return nested
      ? "Use letters, digits, ., _ and -; separate folders with :."
      : "Use letters, digits, ., _ and -.";
  }
  return null;
}

/** A starting file for a new definition, with the frontmatter keys the parser reads. */
export function extensionTemplate(kind: ExtensionKind, name: string): string {
  const title = name.split(":").pop() || name;
  switch (kind) {
    case "agents":
      return `---\nname: ${title}\ndescription: When to use this agent; the model reads this when it picks a subagent.\ntools: Read, Grep, Glob\n# model: fast            # inherit, main, fast, review, or a model id\n# permissionMode: default\n# isolation: shared      # shared or worktree\n---\n\nYou are a specialist. Describe the role, the steps to follow, and what to report back.\n`;
    case "commands":
      return `---\ndescription: What /${name} does, shown in the command menu.\nargument-hint: "[target]"\n# allowed-tools: Bash(git status:*)\n---\n\nDo the task for $ARGUMENTS.\n`;
    case "skills":
      return `---\nname: ${title}\ndescription: When to use this skill; the model reads this to decide.\n# allowed-tools: Read, Bash(npm run build:*)\n---\n\n# ${title}\n\nStep-by-step instructions. Files next to SKILL.md can be referenced by relative path.\n`;
    case "rules":
      return `---\ndescription: What this rule covers.\nglobs: src/**/*.ts\nalwaysApply: false\n---\n\nGuidance applied when matching files are involved.\n`;
    case "output-styles":
      return `---\nname: ${title}\ndescription: How responses should read.\n---\n\nDescribe the tone, structure and length of responses.\n`;
  }
}

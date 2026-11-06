import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import { compactJson, firstLine, list, num, str } from "./toolInput";

export type ToolFamily =
  | "edit"
  | "write"
  | "bash"
  | "read"
  | "search"
  | "web"
  | "agent"
  | "todo"
  | "question"
  | "plan"
  | "job"
  | "mcp"
  | "generic";

export interface ToolMeta {
  family: ToolFamily;
  label: string;
  /** MCP server name for `mcp__server__tool`. */
  server?: string;
}

const FAMILIES: Record<string, ToolFamily> = {
  Edit: "edit",
  MultiEdit: "edit",
  NotebookEdit: "edit",
  Write: "write",
  Bash: "bash",
  Read: "read",
  NotebookRead: "read",
  Glob: "search",
  Grep: "search",
  LS: "search",
  WebFetch: "web",
  WebSearch: "web",
  Agent: "agent",
  Task: "agent",
  TodoWrite: "todo",
  AskUserQuestion: "question",
  ExitPlanMode: "plan",
  BashOutput: "job",
  JobOutput: "job",
  KillShell: "job",
  KillBash: "job",
  JobKill: "job",
};

export function toolMeta(name: string): ToolMeta {
  const mcp = /^mcp__(.+?)__(.+)$/.exec(name);
  if (mcp) return { family: "mcp", label: mcp[2], server: mcp[1] };
  return { family: FAMILIES[name] ?? "generic", label: name || "Tool" };
}

function searchSubject(name: string, input: JsonValue): string {
  const pattern = str(input, "pattern", "query");
  const where = str(input, "path", "glob");
  if (name === "LS") return str(input, "path") || ".";
  return where && pattern ? `${pattern} in ${where}` : pattern || where;
}

/** One-line argument shown next to the tool name ("src/main.rs", "npm test", …). */
export function toolSubject(name: string, input: JsonValue): string {
  const meta = toolMeta(name);
  switch (meta.family) {
    case "edit":
    case "write":
      return str(input, "file_path", "notebook_path", "path");
    case "read": {
      const path = str(input, "file_path", "notebook_path", "path");
      const offset = num(input, "offset");
      const limit = num(input, "limit");
      if (offset === null && limit === null) return path;
      const from = offset ?? 1;
      return limit === null ? `${path}:${from}+` : `${path}:${from}–${from + limit - 1}`;
    }
    case "bash":
      return firstLine(str(input, "command"));
    case "search":
      return searchSubject(name, input);
    case "web":
      return str(input, "url", "query");
    case "agent":
      return str(input, "description") || firstLine(str(input, "prompt"));
    case "todo": {
      const count = list(input, "todos").length;
      return `${count} item${count === 1 ? "" : "s"}`;
    }
    case "question": {
      const first = list(input, "questions")[0];
      return firstLine(str(first, "header", "question"));
    }
    case "plan":
      return firstLine(str(input, "plan"));
    case "job":
      return str(input, "bash_id", "shell_id", "job_id", "id");
    default:
      return firstLine(str(input, "path", "file_path", "url", "query", "command", "name")) || compactJson(input);
  }
}

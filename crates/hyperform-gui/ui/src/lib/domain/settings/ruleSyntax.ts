/** Inline hints for permission rules, following the policy engine's grammar. */

export type HintLevel = "ok" | "info" | "warn" | "error";

export interface RuleHint {
  level: HintLevel;
  message: string;
}

/** File tools that take path specifiers besides the `Read` and `Edit` groups. */
const FILE_TOOLS = ["Write", "MultiEdit", "NotebookEdit", "ApplyAgentChanges", "Glob", "Grep", "LSP", "NotebookRead"];

const KNOWN_TOOLS = [
  "Bash",
  "Read",
  "Edit",
  ...FILE_TOOLS,
  "WebFetch",
  "WebSearch",
  "Agent",
  "Task",
  "Skill",
  "TodoWrite",
  "ExitPlanMode",
  "AskUserQuestion",
  "JobOutput",
  "JobKill",
];

const EXAMPLES = "Read, Bash(npm test:*), Edit(src/**), WebFetch(domain:example.com), mcp__github__create_issue";

const hint = (level: HintLevel, message: string): RuleHint => ({ level, message });
const error = (message: string) => hint("error", message);
const ok = (message: string) => hint("ok", message);

/** What the rule matches, or why the policy engine would reject it. */
export function ruleHint(input: string): RuleHint {
  const text = input.trim();
  if (!text) return hint("info", `Examples: ${EXAMPLES}`);
  let tool = text;
  let spec: string | null = null;
  const open = text.indexOf("(");
  if (open >= 0) {
    if (!text.endsWith(")")) return error("Missing closing parenthesis.");
    spec = text.slice(open + 1, -1).trim();
    if (!spec) return error("Empty specifier: remove the parentheses or add a pattern.");
    tool = text.slice(0, open).trimEnd();
  }
  if (!tool) return error("Missing tool name.");
  if (!/^[A-Za-z0-9_.*-]+$/.test(tool)) return error("Tool names use letters, digits, _, - and .");
  if (tool.startsWith("mcp__")) return mcpHint(tool.slice(5), spec);
  if (tool.includes("*")) return error("Wildcards are only supported as mcp__server__*.");
  switch (tool) {
    case "Bash":
      return bashHint(spec);
    case "WebFetch":
      return webFetchHint(spec);
    case "WebSearch":
      return spec ? error("WebSearch rules do not take a specifier.") : ok("Every web search.");
    case "Agent":
    case "Task":
      return ok(spec ? `Subagents of type “${spec}”.` : "Every subagent.");
    case "Skill":
      return ok(spec ? `The “${spec}” skill.` : "Every skill.");
  }
  if (tool === "Read" || tool === "Edit" || FILE_TOOLS.includes(tool)) return pathHint(tool, spec);
  if (spec) return error(`${tool} rules do not take a specifier.`);
  const known = KNOWN_TOOLS.find((name) => name.toLowerCase() === tool.toLowerCase());
  if (known && known !== tool) return hint("warn", `Tool names are case-sensitive: did you mean ${known}?`);
  return known ? ok(`Every ${tool} call.`) : hint("info", `Matches a tool named ${tool} exactly.`);
}

function bashHint(spec: string | null): RuleHint {
  if (spec === null) return ok("Every shell command.");
  const colon = spec.endsWith(":*");
  const star = !colon && spec.endsWith("*");
  const body = (colon ? spec.slice(0, -2) : star ? spec.slice(0, -1) : spec).trimEnd();
  if (!body) return ok("Same as Bash: every shell command.");
  const compound = /&&|\|\||[;|<>`]|\$\(/.test(body);
  if ((colon || star) && compound) return error("A prefix rule must name one simple command.");
  if (star) return hint("info", `Read as the prefix Bash(${body}:*): commands starting with “${body}”.`);
  if (colon) return ok(`Commands starting with the words “${body}”.`);
  if (compound) return hint("info", "Compound commands only match when run exactly as written.");
  return ok(`Exactly “${body}”. Add :* to also match longer commands.`);
}

function pathHint(tool: string, spec: string | null): RuleHint {
  if (spec === null) {
    if (tool === "Read") return ok("Every read action (Read, Glob, Grep, LSP, …).");
    if (tool === "Edit") return ok("Every write action (Write, Edit, MultiEdit, NotebookEdit, …).");
    return ok(`Every ${tool} call.`);
  }
  if (spec.startsWith("!")) return error("Negated patterns are not supported.");
  if (spec.startsWith("~") && spec !== "~" && !spec.startsWith("~/")) {
    return error("Only ~/ home paths are supported.");
  }
  if (count(spec, "[") !== count(spec, "]") || count(spec, "{") !== count(spec, "}")) {
    return error("Unbalanced [ ] or { } in the pattern.");
  }
  if (spec.startsWith("//")) return ok("An absolute path pattern.");
  if (spec.startsWith("~")) return ok("Relative to your home folder.");
  if (spec.startsWith("/")) {
    return hint("info", "A single leading / matches both absolute paths and paths from the project root; use // for absolute only.");
  }
  const body = spec.replace(/^\.\//, "").replace(/\/+$/, "");
  if (!body.includes("/")) return ok("Matches at any depth in the project, like .gitignore.");
  return ok("Relative to the project root, like .gitignore.");
}

function webFetchHint(spec: string | null): RuleHint {
  if (spec === null) return ok("Every web fetch.");
  if (!spec.startsWith("domain:")) return error("WebFetch rules take domain:<host>, e.g. WebFetch(domain:example.com).");
  const value = spec.slice("domain:".length).trim();
  const host = value.startsWith("*.") ? value.slice(2) : value;
  const domain = host.replace(/\.+$/, "").toLowerCase();
  if (!domain || !/^[a-z0-9._-]+$/.test(domain)) return error(`Invalid domain “${value}”.`);
  return ok(`${domain} and its subdomains.`);
}

function mcpHint(rest: string, spec: string | null): RuleHint {
  if (spec !== null) return error("MCP rules do not take a specifier.");
  const split = rest.indexOf("__");
  const server = split >= 0 ? rest.slice(0, split) : rest;
  const tool = split >= 0 ? rest.slice(split + 2) : null;
  if (tool === "") return error("Missing MCP tool name.");
  if (tool !== null && tool !== "*" && tool.includes("*")) return error("Only mcp__server__* wildcards are supported.");
  if (!server || server.includes("*")) return error("Missing MCP server name.");
  if (tool === null || tool === "*") return ok(`Every tool of the ${server} MCP server.`);
  return ok(`The ${tool} tool of the ${server} MCP server.`);
}

function count(text: string, char: string): number {
  return text.split(char).length - 1;
}

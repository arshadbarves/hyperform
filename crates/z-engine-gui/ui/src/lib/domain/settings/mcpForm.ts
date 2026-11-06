import type { McpServerConfig } from "../../protocol/config/McpServerConfig";
import { formatArgs, formatKeyValues, formatList, optionalText, parseArgs, parseKeyValues, parseList } from "./formText";
import { LIMITS, parseNumber } from "./limits";
import { isRecord, stringList } from "./provenance";

export type McpTransport = "stdio" | "http";

export const DEFAULT_MCP_TIMEOUT = 60;

export const MCP_SERVERS_PATH = ["mcp", "servers"] as const;

/** The editor's text fields for one server. */
export interface McpForm {
  name: string;
  transport: McpTransport;
  command: string;
  args: string;
  env: string;
  cwd: string;
  url: string;
  headers: string;
  enabled: boolean;
  disabledTools: string;
  timeout: string;
}

function stringMap(value: unknown): Record<string, string> {
  if (!isRecord(value)) return {};
  return Object.fromEntries(Object.entries(value).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
}

const optionalString = (value: unknown) => (typeof value === "string" && value.trim() ? value : null);

/** A layer file's server entry with the loader's defaults. */
export function normalizeMcpServer(raw: unknown): McpServerConfig {
  const entry = isRecord(raw) ? raw : {};
  return {
    command: optionalString(entry.command),
    args: stringList(entry.args),
    env: stringMap(entry.env),
    cwd: optionalString(entry.cwd),
    url: optionalString(entry.url),
    headers: stringMap(entry.headers),
    enabled: typeof entry.enabled === "boolean" ? entry.enabled : true,
    disabled_tools: stringList(entry.disabled_tools),
    timeout_secs: typeof entry.timeout_secs === "number" ? entry.timeout_secs : DEFAULT_MCP_TIMEOUT,
  };
}

/** Why the server cannot start; the same rule as the config crate. */
export function transportError(server: McpServerConfig): string | null {
  const command = Boolean(server.command?.trim());
  const url = Boolean(server.url?.trim());
  if (command && url) return "Set only one of command and URL.";
  return command || url ? null : "Set a command or a URL.";
}

export function mcpNameError(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Enter a server name.";
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(trimmed) || trimmed.includes("__")) {
    return "Use letters, digits, - and single _ (tools are named mcp__<server>__<tool>).";
  }
  return taken.includes(trimmed) ? `A server named ${trimmed} already exists in this file.` : null;
}

export function emptyMcpForm(): McpForm {
  return {
    name: "",
    transport: "stdio",
    command: "",
    args: "",
    env: "",
    cwd: "",
    url: "",
    headers: "",
    enabled: true,
    disabledTools: "",
    timeout: String(DEFAULT_MCP_TIMEOUT),
  };
}

export function mcpFormFrom(name: string, server: McpServerConfig): McpForm {
  return {
    name,
    transport: server.url ? "http" : "stdio",
    command: server.command ?? "",
    args: formatArgs(server.args),
    env: formatKeyValues(server.env, "="),
    cwd: server.cwd ?? "",
    url: server.url ?? "",
    headers: formatKeyValues(server.headers, ":"),
    enabled: server.enabled,
    disabledTools: formatList(server.disabled_tools),
    timeout: String(server.timeout_secs),
  };
}

/** The server a form describes; only the chosen transport's fields are kept. */
export function mcpServerFrom(form: McpForm): { server: McpServerConfig } | { error: string } {
  const timeout = parseNumber(form.timeout, LIMITS.timeoutSecs);
  if (!timeout.ok) return { error: `Timeout: ${timeout.error}` };
  const stdio = form.transport === "stdio";
  const args = stdio ? parseArgs(form.args) : { args: [], error: null };
  if (args.error) return { error: `Arguments: ${args.error}` };
  const env = stdio ? parseKeyValues(form.env, "=") : { values: {}, error: null };
  if (env.error) return { error: `Environment: ${env.error}` };
  const headers = stdio ? { values: {}, error: null } : parseKeyValues(form.headers, ":");
  if (headers.error) return { error: `Headers: ${headers.error}` };
  const server: McpServerConfig = {
    command: stdio ? optionalText(form.command) : null,
    args: args.args,
    env: env.values,
    cwd: stdio ? optionalText(form.cwd) : null,
    url: stdio ? null : optionalText(form.url),
    headers: headers.values,
    enabled: form.enabled,
    disabled_tools: parseList(form.disabledTools),
    timeout_secs: timeout.value ?? DEFAULT_MCP_TIMEOUT,
  };
  const problem = transportError(server);
  return problem ? { error: problem } : { server };
}

/** One line describing how the server is reached. */
export function serverSummary(server: McpServerConfig): string {
  if (server.url) return server.url;
  return [server.command ?? "", formatArgs(server.args)].filter(Boolean).join(" ");
}

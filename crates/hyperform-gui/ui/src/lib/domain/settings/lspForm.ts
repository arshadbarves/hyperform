import type { LspServerConfig } from "../../protocol/config/LspServerConfig";
import { formatArgs, formatList, parseArgs, parseList } from "./formText";
import { isRecord, stringList } from "./provenance";

export const LSP_SERVERS_PATH = ["lsp", "servers"] as const;

export interface LspForm {
  name: string;
  command: string;
  args: string;
  extensions: string;
  rootMarkers: string;
  enabled: boolean;
}

/** A layer file's server entry with the loader's defaults. */
export function normalizeLspServer(raw: unknown): LspServerConfig {
  const entry = isRecord(raw) ? raw : {};
  return {
    command: typeof entry.command === "string" ? entry.command : "",
    args: stringList(entry.args),
    extensions: stringList(entry.extensions),
    root_markers: stringList(entry.root_markers),
    enabled: typeof entry.enabled === "boolean" ? entry.enabled : true,
  };
}

export function emptyLspForm(): LspForm {
  return { name: "", command: "", args: "", extensions: "", rootMarkers: "", enabled: true };
}

export function lspFormFrom(name: string, server: LspServerConfig): LspForm {
  return {
    name,
    command: server.command,
    args: formatArgs(server.args),
    extensions: formatList(server.extensions),
    rootMarkers: formatList(server.root_markers),
    enabled: server.enabled,
  };
}

export function lspNameError(name: string, taken: readonly string[]): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "Enter a name, such as rust or typescript.";
  if (!/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(trimmed)) return "Names use letters, digits, _, - and .";
  return taken.includes(trimmed) ? `A server named ${trimmed} already exists in this file.` : null;
}

/** Extensions are stored without the dot. */
export function lspServerFrom(form: LspForm): { server: LspServerConfig } | { error: string } {
  const command = form.command.trim();
  if (!command) return { error: "Enter the language server command." };
  const args = parseArgs(form.args);
  if (args.error) return { error: `Arguments: ${args.error}` };
  const extensions = parseList(form.extensions).map((ext) => ext.replace(/^\./, ""));
  if (extensions.length === 0) return { error: "List the file extensions it serves, such as rs." };
  return {
    server: {
      command,
      args: args.args,
      extensions,
      root_markers: parseList(form.rootMarkers),
      enabled: form.enabled,
    },
  };
}

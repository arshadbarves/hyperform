import { listAgents, listCommands, listFiles, type AgentCard, type SlashCommandInfo } from "../commands";
import { mergeCommands } from "../domain/slashCommands";

const FILE_LIMIT = 40;

/** Slash commands and custom agents per project root, fetched once per root. */
class CatalogStore {
  #commands: Record<string, SlashCommandInfo[]> = $state.raw({});
  #agents: Record<string, AgentCard[]> = $state.raw({});
  #inflight = new Set<string>();

  commandsFor(root: string | null | undefined): SlashCommandInfo[] {
    return mergeCommands(root ? this.#commands[root] : null);
  }

  agentsFor(root: string | null | undefined): AgentCard[] {
    return root ? (this.#agents[root] ?? []) : [];
  }

  async ensure(root: string | null | undefined) {
    if (!root || root in this.#commands || this.#inflight.has(root)) return;
    this.#inflight.add(root);
    try {
      const [commands, agents] = await Promise.allSettled([listCommands(root), listAgents(root)]);
      this.#commands = { ...this.#commands, [root]: commands.status === "fulfilled" ? commands.value : [] };
      this.#agents = { ...this.#agents, [root]: agents.status === "fulfilled" ? agents.value : [] };
    } finally {
      this.#inflight.delete(root);
    }
  }

  /** Drop the cache after `reloadExtensions` so new commands and agents show up. */
  async reload(root: string | null | undefined) {
    if (!root) return;
    const commands = { ...this.#commands };
    const agents = { ...this.#agents };
    delete commands[root];
    delete agents[root];
    this.#commands = commands;
    this.#agents = agents;
    await this.ensure(root);
  }
}

export const catalogs = new CatalogStore();

/** `@` file search in the session's project. */
export async function searchFiles(root: string | null | undefined, query: string): Promise<string[]> {
  if (!root) return [];
  try {
    return await listFiles(root, query, FILE_LIMIT);
  } catch (e) {
    console.error("list_files failed", e);
    return [];
  }
}

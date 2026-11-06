import type { SlashCommandInfo } from "../commands/engine";

/** Markdown for `/help`: commands grouped by where they run, then shortcuts. */
export function helpMarkdown(commands: SlashCommandInfo[], mod: string): string {
  const row = (c: SlashCommandInfo) =>
    `- \`/${c.name}${c.argumentHint ? ` ${c.argumentHint}` : ""}\` — ${c.description}`;
  const section = (title: string, list: SlashCommandInfo[]) =>
    list.length > 0 ? [`**${title}**`, ...list.map(row), ""] : [];
  const byName = (a: SlashCommandInfo, b: SlashCommandInfo) => a.name.localeCompare(b.name);
  return [
    ...section("Agent commands", commands.filter((c) => c.kind !== "ui").sort(byName)),
    ...section("App commands", commands.filter((c) => c.kind === "ui").sort(byName)),
    "**Keys**",
    "- `Enter` send · `Shift+Enter` newline",
    "- `Enter` while working queues a steering message",
    `- \`${mod}Enter\` interrupt and send now · \`Esc\` cancel the turn`,
    "- `Shift+Tab` turn plan mode on or off",
    "- `@` files and agents · `/` commands · `!` shell · `#` remember",
    `- \`${mod}K\` palette · \`${mod}N\` new chat · \`${mod}B\` sidebar · \`${mod}D\` changes`,
  ].join("\n");
}

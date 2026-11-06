/** Text fields of the settings forms and the values they stand for. */

/** Trimmed text, or null when blank (an unset optional value). */
export function optionalText(text: string): string | null {
  const trimmed = text.trim();
  return trimmed ? trimmed : null;
}

/** Items separated by commas or new lines; blanks and repeats are dropped. */
export function parseList(text: string): string[] {
  const items = text.split(/[,\n]/).map((item) => item.trim());
  return [...new Set(items.filter(Boolean))];
}

export function formatList(items: readonly string[]): string {
  return items.join(", ");
}

/** Shell-style words: spaces separate, quotes group, `\` escapes the next character. */
export function parseArgs(text: string): { args: string[]; error: string | null } {
  const args: string[] = [];
  let current = "";
  let quote: '"' | "'" | null = null;
  let started = false;
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (quote) {
      if (char === quote) quote = null;
      else if (char === "\\" && quote === '"' && i + 1 < text.length) current += text[++i];
      else current += char;
    } else if (char === '"' || char === "'") {
      quote = char;
      started = true;
    } else if (char === "\\" && i + 1 < text.length) {
      current += text[++i];
      started = true;
    } else if (/\s/.test(char)) {
      if (started) args.push(current);
      current = "";
      started = false;
    } else {
      current += char;
      started = true;
    }
  }
  if (quote) return { args: [], error: `Close the ${quote} quote.` };
  if (started) args.push(current);
  return { args, error: null };
}

/** The inverse of `parseArgs`: words with spaces or quotes are double-quoted. */
export function formatArgs(args: readonly string[]): string {
  return args
    .map((arg) => (arg === "" || /[\s"'\\]/.test(arg) ? `"${arg.replace(/(["\\])/g, "\\$1")}"` : arg))
    .join(" ");
}

/** `KEY=value` (env) or `Name: value` (headers) lines; the first separator splits. */
export function parseKeyValues(
  text: string,
  separator: "=" | ":",
): { values: Record<string, string>; error: string | null } {
  const values: Record<string, string> = {};
  const lines = text.split("\n").map((line) => line.trim());
  for (const [index, line] of lines.entries()) {
    if (!line || line.startsWith("#")) continue;
    const at = line.indexOf(separator);
    const key = at > 0 ? line.slice(0, at).trim() : "";
    if (!key) {
      const example = separator === "=" ? "NAME=value" : "Name: value";
      return { values: {}, error: `Line ${index + 1}: write ${example}.` };
    }
    values[key] = line.slice(at + 1).trim();
  }
  return { values, error: null };
}

export function formatKeyValues(values: Record<string, string>, separator: "=" | ":"): string {
  const join = separator === "=" ? "=" : ": ";
  return Object.entries(values)
    .map(([key, value]) => `${key}${join}${value}`)
    .join("\n");
}

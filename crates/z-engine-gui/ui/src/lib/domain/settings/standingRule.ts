/** A standing rule saved from a suggestion card, as one list item at the end of an instruction file. */

function normalized(text: string): string {
  return text.trim().replace(/\s+/g, " ").replace(/[.!]+$/, "").toLowerCase();
}

/** Whether `content` already holds `rule`, ignoring case, spacing and a final period. */
export function hasRule(content: string, rule: string): boolean {
  return normalized(content).includes(normalized(rule));
}

/** `content` with `rule` appended as a `- ` list item on its own line. */
export function appendRule(content: string, rule: string): string {
  const line = `- ${rule.trim().replace(/\s+/g, " ")}`;
  const body = content.replace(/\s+$/, "");
  return body ? `${body}\n${line}\n` : `${line}\n`;
}

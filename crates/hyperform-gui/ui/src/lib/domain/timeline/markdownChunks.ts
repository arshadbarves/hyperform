const FENCE = /^ {0,3}(`{3,}|~{3,})/;
/** Link reference and footnote definitions reach across blocks. */
const DEFINITION = /^ {0,3}\[[^\]\n]+\]:/m;
/** A line that may continue the block above it: indented, a list item or a quote. */
const CONTINUES = /^(?:[ \t]|[-*+][ \t]|\d{1,9}[.)][ \t]|>)/;

/**
 * Splits markdown into top-level blocks that render the same one at a time,
 * so a streaming reply re-parses only its last block. It splits only at a
 * blank line outside a code fence, and never before a line that may continue
 * the block above. Text with reference definitions stays whole. Joining the
 * chunks with "\n" gives back the text.
 */
export function markdownChunks(text: string): string[] {
  if (DEFINITION.test(text)) return [text];
  const lines = text.split("\n");
  const chunks: string[] = [];
  let from = 0;
  let fence = "";
  let afterBlank = false;
  lines.forEach((line, i) => {
    const mark = FENCE.exec(line)?.[1] ?? "";
    if (fence) {
      const closes = mark[0] === fence[0] && mark.length >= fence.length && !line.trim().slice(mark.length).trim();
      if (closes) fence = "";
      return;
    }
    if (!line.trim()) {
      afterBlank = true;
      return;
    }
    if (afterBlank && i > from && !CONTINUES.test(line)) {
      chunks.push(lines.slice(from, i).join("\n"));
      from = i;
    }
    afterBlank = false;
    fence = mark;
  });
  chunks.push(lines.slice(from).join("\n"));
  return chunks;
}

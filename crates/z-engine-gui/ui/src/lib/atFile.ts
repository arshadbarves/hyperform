/** Given textarea text and caret position, return the active `@token`
 * query (text after an `@` that starts a token at the caret), or null.
 * A token start is string-start or a preceding whitespace. */
export function activeAtToken(text: string, caret: number): string | null {
  const before = text.slice(0, caret);
  const match = before.match(/(^|\s)@([^\s]*)$/);
  return match ? match[2] : null;
}

/** Remove the active `@token` ending at `caret` (used when the pick
 * becomes an attachment chip instead of inline text). Keeps any
 * separator whitespace before the token. */
export function stripAtToken(text: string, caret: number): { text: string; caret: number } {
  return replaceAtToken(text, caret, "");
}

/** Replace the active `@token` ending at `caret` with `replacement`
 * (for example `@src/main.rs `), returning the new text and caret. */
export function replaceAtToken(
  text: string,
  caret: number,
  replacement: string,
): { text: string; caret: number } {
  const before = text.slice(0, caret);
  const after = text.slice(caret);
  const m = before.match(/(^|\s)@([^\s]*)$/);
  if (!m || m.index === undefined) return { text, caret };
  const prefix = before.slice(0, m.index + m[1].length) + replacement;
  return { text: prefix + after, caret: prefix.length };
}

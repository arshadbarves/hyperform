Searches file contents with regular expressions, built on ripgrep.

- Use Grep for every content search; never run `grep` or `rg` through Bash.
- `pattern` uses ripgrep (Rust regex) syntax, such as `log.*Error` or `fn\s+\w+`. Escape literal metacharacters: `interface\{\}` finds `interface{}`, and `\(` finds `(`.
- `path` is a file or directory to search; the default is the project root. Narrow the files with `glob` (`*.js`, `**/*.{ts,tsx}`, with a leading `!` to exclude) or `type` (a ripgrep file type such as `rust`, `py`, `js`, or `go`, which is more efficient than `glob` for standard types). Files ignored by `.gitignore` are skipped.
- `output_mode` chooses the result:
  - `files_with_matches` (the default) lists the matching files. Use it to locate code.
  - `content` shows matching lines as `path:line:text`. Add context with `-A` (lines after), `-B` (lines before), or `-C` (both); `-n` (line numbers) defaults to true.
  - `count` shows the number of matching lines per file.
- `-i` makes the search case-insensitive.
- A pattern matches within single lines by default. Set `multiline` to let patterns span lines and `.` match newlines, as in `struct \w+ \{[\s\S]*?field`.
- Page through large results with `head_limit` (keep the first N lines or entries) and `offset` (skip the first N); the result says when more are available.
- For open-ended searches that need several rounds, use an `explore` agent instead.

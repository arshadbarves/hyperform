Finds files by path with a glob pattern. It is fast on projects of any size.

- Glob syntax: `*` matches within one path segment, `**` across directories, `?` one character, `{a,b}` either alternative, and `[abc]` one character from a set. Examples: `**/*.rs`, `src/**/*.test.ts`, `*.{json,toml}`, `**/Cargo.toml`.
- The pattern is matched against paths relative to `path`, which defaults to the project root. Pass `path` to search one directory; omit it rather than passing an empty value.
- Files ignored by `.gitignore` are skipped. Results list files (not directories), most recently modified first, up to 100 of them; when the result is truncated, use a more specific pattern or path.
- Use Grep to search file contents. For open-ended exploration that needs many rounds of globbing and grepping, use an `explore` agent.
- Make independent Glob calls in the same message to run them in parallel.

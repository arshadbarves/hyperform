Queries a language server for exact code intelligence, where one supports the file's language.

- `operation` selects the query:
  - `definition`, `references`, `hover`, and `implementations` look up the symbol at `file_path`, `line`, and `character`.
  - `incomingCalls` and `outgoingCalls` show the call hierarchy of the function at that position.
  - `documentSymbols` lists the symbols defined in `file_path`.
  - `workspaceSymbols` finds symbols across the project matching `query`.
  - `diagnostics` reports errors and warnings for `file_path`, or for the whole project when it is omitted.
  - `renamePreview` lists the edits that renaming the symbol at the position to `new_name` would make. Nothing is changed; apply the edits with Edit or MultiEdit.
- `line` and `character` are 1-based, as shown in Read output and editors.
- Prefer it to text search for definitions, references, implementations, and call hierarchies: it is exact where Grep is approximate. Check `diagnostics` after editing.
- If no language server supports the file, the call fails; use Grep and Read instead.

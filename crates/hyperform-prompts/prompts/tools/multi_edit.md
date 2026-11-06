Makes several replacements in one file in a single atomic operation. Prefer it to repeated Edit calls on the same file.

- Every rule of Edit applies to each entry of `edits`: read the file first, make `old_string` match exactly and uniquely (or set `replace_all`), and make `new_string` differ from `old_string`.
- Edits are applied in order, each to the result of the previous one. Make sure an earlier edit does not change text that a later edit still needs to match.
- The operation is all-or-nothing: if any edit fails, none are applied, and the error says which edit failed and why.
- To create a new file, give the first edit an empty `old_string`; its `new_string` becomes the file's content, and later edits apply to it.
- The result shows every changed region numbered like Read.

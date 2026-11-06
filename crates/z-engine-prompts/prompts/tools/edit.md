Replaces text in a file with an exact string replacement.

- Read the file first. The edit is refused if you have not read the file, or if it changed after you last read it.
- `old_string` must match the file exactly, including whitespace and indentation. When you copy text from Read output, take only what follows the tab after the line number, never the number itself.
- `old_string` must occur exactly once. If it occurs several times the edit fails and reports the count: add surrounding lines until it is unique, or set `replace_all` to change every occurrence (for example to rename a variable throughout the file).
- `new_string` must differ from `old_string`. An empty `new_string` deletes the text.
- If `old_string` is not found exactly, a region that differs only in whitespace or indentation, or a single near-identical region, may be matched instead. The result then says which lines matched and shows the edited region; check it. Several similar regions are never guessed between: the edit fails and asks for more context.
- An empty `old_string` creates a new file containing `new_string`, or fills an existing empty file.
- The result shows the edited lines numbered like Read, so you do not need to read the file again to check the edit.
- Use MultiEdit for several changes to one file, and NotebookEdit for Jupyter notebooks.

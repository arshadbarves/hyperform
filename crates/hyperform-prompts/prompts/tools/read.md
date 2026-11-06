Reads a file from the local filesystem.

- `file_path` is the file to read: an absolute path, or a path relative to the project root. `~` expands to the home directory.
- Text comes back in `cat -n` format: each line starts with its line number (counting from 1) and a tab. The numbers are not part of the file; never copy them into an Edit `old_string`.
- By default up to 2000 lines are returned from the start of the file. For large files, pass `offset` (the line to start from) and `limit` (how many lines) to read the part you need; the result says when more lines follow. Lines longer than 2000 characters are cut off.
- Images (PNG, JPEG, GIF, WebP), such as screenshots and diagrams, are returned so you can see them when the model supports images.
- PDFs are returned as text, page by page, at most 20 pages per call. Pass `pages` (for example `"1-5"` or `"12"`) to choose them; `pages` applies only to PDFs. Scanned PDFs may contain no extractable text.
- Jupyter notebooks (`.ipynb`) are returned as their cells, each with its id, type, source, and outputs. Use those ids with NotebookEdit.
- Read only reads files. To see what a directory contains, use Glob, or `ls` through Bash.
- Read a file before you edit or overwrite it: Edit, MultiEdit, Write, and NotebookEdit refuse files you have not read, or that changed after you read them.
- Read several files at once by making the calls in the same message. It is fine to try a path that may not exist; you get an error back.
- An empty file is reported as empty, and a binary file by its size instead of its contents.

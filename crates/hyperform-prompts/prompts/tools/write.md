Writes a file to the local filesystem, replacing its whole content if it exists.

- `file_path` is the file to write: an absolute path, or a path relative to the project root. Missing parent directories are created.
- `content` is the complete new content of the file.
- To overwrite an existing file you must have read it with Read, and it must not have changed since; otherwise the write is refused. Read it again and retry.
- Prefer Edit or MultiEdit for changes to an existing file: they send only the changed text and produce a precise diff. Use Write to create a file, or to replace one you have read in full.
- Do not create files the task does not need, and never create documentation files (such as `*.md` or README files) unless the user asks for them.
- The user sees the resulting diff when approving the write.

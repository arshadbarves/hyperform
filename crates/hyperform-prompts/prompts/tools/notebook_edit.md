Edits one cell of a Jupyter notebook (`.ipynb`): replaces its source, inserts a new cell, or deletes it.

- Read the notebook first; Read shows every cell with its id. The edit is refused if you have not read the notebook, or if it changed after you read it.
- `notebook_path` is the notebook file. `cell_id` identifies a cell by the id Read shows, such as `a1b2c3d4`, or as `cell-N` (N counting from 0) in notebooks without cell ids.
- `edit_mode` is `replace` (the default), `insert`, or `delete`:
  - `replace` sets the source of cell `cell_id` to `new_source`. Pass `cell_type` to change the cell's type. A replaced code cell loses its outputs, since they no longer match its code.
  - `insert` adds a new cell with `new_source` right after cell `cell_id`, or at the top of the notebook when `cell_id` is omitted. `cell_type` (`code` or `markdown`) is required.
  - `delete` removes cell `cell_id`; `new_source` is ignored.
- Everything else in the notebook (metadata, other cells, and their outputs) is preserved.
- Use Edit for files that are not notebooks.

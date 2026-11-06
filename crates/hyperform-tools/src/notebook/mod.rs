//! Jupyter notebooks, shared by `Read` (rendering) and `NotebookEdit`.

mod model;
mod render;

pub(crate) use model::{CellType, Notebook, cell_id, joined};
pub(crate) use render::render;

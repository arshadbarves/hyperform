//! Format-specific views behind the `Read` tool.

mod media;
mod pdf;
mod text;

pub(crate) use media::{binary, human_size, image, notebook};
pub(crate) use pdf::{parse_pages, read_pdf};
pub(crate) use text::window;

//! v1 -> v2 settings import. A file without a `schema` key is v1; v1 files
//! are read and converted, never modified.

mod convert;
mod import;

pub(crate) use convert::convert_v1;
pub use import::{MigrationOutcome, import_v1_file};
pub(crate) use import::{as_v2, read_legacy};

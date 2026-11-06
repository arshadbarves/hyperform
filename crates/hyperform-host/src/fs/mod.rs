//! Files: atomic writes, path arithmetic, bounded reads, read tracking, and
//! per-path edit locks.

mod atomic;
mod locks;
mod paths;
mod read;
mod tracker;

pub use atomic::{atomic_write, atomic_write_sync};
pub use locks::PathLocks;
pub use paths::{expand_tilde, is_within, normalize, relative_display, resolve};
pub use read::{FileKind, TextFile, read_text, sniff};
pub use tracker::{FileTracker, Freshness};

pub(crate) use read::{image_media_type, is_pdf};

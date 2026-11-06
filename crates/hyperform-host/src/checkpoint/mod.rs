//! Code checkpoints in a shadow git repository: snapshot the whole working
//! tree, list changes between snapshots (or from one to the working tree),
//! and restore one.

mod compare;
mod exclude;
mod restore;
mod shadow;

pub use restore::{ChangeKind, PathChange, RestoreReport};
pub use shadow::{ShadowRepo, project_key};

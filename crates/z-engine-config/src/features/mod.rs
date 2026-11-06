//! The Experimental-to-Stable lifecycle: every new feature has an id, a
//! registry entry with owner and graduation criteria, and a mode in
//! `[experimental]` (see `Settings::feature`).

mod id;
mod mode;
mod registry;

pub use id::FeatureId;
pub use mode::FeatureMode;
pub use registry::{FEATURES, FeatureGroup, FeatureSpec, FeatureStage};

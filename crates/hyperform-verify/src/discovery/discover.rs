//! Discovery: one bounded walk collects manifests, then each ecosystem
//! turns its manifests into roots, checks and notes. Nothing is executed.

use std::path::Path;

use super::collector::Collector;
use super::ecosystems::{cargo, cmake, deno, dotnet, go, gradle, just, make, maven, node, python};
use super::types::{DiscoveryOptions, ProjectProfile};
use super::walk::{Found, walk};

struct Ecosystem {
    is_manifest: fn(&str) -> bool,
    discover: fn(&mut Collector, &[Found]),
}

/// Also the order of roots (and their checks) that share a directory.
const ECOSYSTEMS: &[Ecosystem] = &[
    Ecosystem {
        is_manifest: cargo::is_manifest,
        discover: cargo::discover,
    },
    Ecosystem {
        is_manifest: node::is_manifest,
        discover: node::discover,
    },
    Ecosystem {
        is_manifest: deno::is_manifest,
        discover: deno::discover,
    },
    Ecosystem {
        is_manifest: python::is_manifest,
        discover: python::discover,
    },
    Ecosystem {
        is_manifest: go::is_manifest,
        discover: go::discover,
    },
    Ecosystem {
        is_manifest: gradle::is_manifest,
        discover: gradle::discover,
    },
    Ecosystem {
        is_manifest: maven::is_manifest,
        discover: maven::discover,
    },
    Ecosystem {
        is_manifest: dotnet::is_manifest,
        discover: dotnet::discover,
    },
    Ecosystem {
        is_manifest: cmake::is_manifest,
        discover: cmake::discover,
    },
    Ecosystem {
        is_manifest: make::is_manifest,
        discover: make::discover,
    },
    Ecosystem {
        is_manifest: just::is_manifest,
        discover: just::discover,
    },
];

/// Finds project roots and the checks their manifests suggest under
/// `root`: a breadth-first, gitignore-aware walk bounded by `options`,
/// reading only manifests (each at most `max_manifest_bytes`). Commands
/// are suggestions and are never run here. Blocking; problems become
/// notes rather than errors.
pub fn discover(root: &Path, options: &DiscoveryOptions) -> ProjectProfile {
    let mut collector = Collector::new(root, options);
    let is_manifest = |name: &str| ECOSYSTEMS.iter().any(|e| (e.is_manifest)(name));
    let found = walk(&mut collector, options, &is_manifest);
    for ecosystem in ECOSYSTEMS {
        let mine: Vec<Found> = found
            .iter()
            .filter(|f| (ecosystem.is_manifest)(&f.name))
            .cloned()
            .collect();
        if !mine.is_empty() {
            (ecosystem.discover)(&mut collector, &mine);
        }
    }
    let profile = collector.finish();
    tracing::debug!(
        root = %root.display(),
        roots = profile.roots.len(),
        checks = profile.checks.len(),
        notes = profile.notes.len(),
        "project discovery finished"
    );
    profile
}

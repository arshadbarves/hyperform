//! Temporary project trees for discovery tests.
#![allow(dead_code)]

use std::path::Path;

use z_engine_verify::{CheckSpec, DiscoveryOptions, ProjectProfile, discover};

pub struct Fixture(tempfile::TempDir);

impl Fixture {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let fixture = Self(tempfile::tempdir().unwrap());
        for (path, text) in files {
            fixture.write(path, text);
        }
        fixture
    }

    pub fn write(&self, path: &str, text: &str) {
        let path = self.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn mkdir(&self, path: &str) {
        std::fs::create_dir_all(self.path().join(path)).unwrap();
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }

    pub fn discover(&self) -> ProjectProfile {
        discover(self.path(), &DiscoveryOptions::default())
    }

    pub fn discover_with(&self, options: DiscoveryOptions) -> ProjectProfile {
        discover(self.path(), &options)
    }
}

pub fn ids(profile: &ProjectProfile) -> Vec<&str> {
    profile.checks.iter().map(|c| c.id.as_str()).collect()
}

/// `(path, ecosystem)` of every root.
pub fn roots(profile: &ProjectProfile) -> Vec<(&str, &str)> {
    profile
        .roots
        .iter()
        .map(|r| (r.path.as_str(), r.ecosystem.as_str()))
        .collect()
}

pub fn check<'a>(profile: &'a ProjectProfile, id: &str) -> &'a CheckSpec {
    profile
        .checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id} in {:?}", ids(profile)))
}

pub fn command<'a>(profile: &'a ProjectProfile, id: &str) -> &'a str {
    &check(profile, id).command
}

pub fn has_note(profile: &ProjectProfile, fragment: &str) -> bool {
    profile.notes.iter().any(|note| note.contains(fragment))
}

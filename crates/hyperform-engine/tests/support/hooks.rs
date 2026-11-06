//! Small shell hook scripts and their settings entries for tests.

use std::path::Path;

/// Writes a POSIX shell hook script into `dir`; returns its command line.
pub fn hook_script(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
    format!("sh {}", path.display())
}

/// `[[hooks.<event>]]` with an optional matcher, as settings TOML.
pub fn hook_toml(event: &str, matcher: Option<&str>, command: &str) -> String {
    let matcher = matcher
        .map(|matcher| format!("matcher = \"{matcher}\"\n"))
        .unwrap_or_default();
    format!("\n[[hooks.{event}]]\n{matcher}command = \"{command}\"\ntimeout_secs = 20\n")
}

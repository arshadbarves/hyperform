//! The environment agent commands see: an allowlist of the parent's
//! variables plus fixed non-interactive settings, never the full parent env.

use std::collections::BTreeMap;

/// Inherited when set (v1 allowlist).
const INHERITED: &[&str] = &[
    "PATH", "HOME", "SHELL", "TERM", "LANG", "LC_ALL", "TMPDIR", "USER", "LOGNAME",
];

#[cfg(windows)]
const INHERITED_WINDOWS: &[&str] = &[
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "TEMP",
    "TMP",
    "PATHEXT",
    "COMSPEC",
    "SystemRoot",
    "windir",
    "APPDATA",
    "LOCALAPPDATA",
    "PSModulePath",
    "Path",
];

#[cfg(not(windows))]
const INHERITED_WINDOWS: &[&str] = &[];

/// Always set so commands never block on a prompt or a pager.
const FIXED: &[(&str, &str)] = &[
    ("GIT_TERMINAL_PROMPT", "0"),
    ("PAGER", "cat"),
    ("GIT_PAGER", "cat"),
    ("HYPERFORM", "1"),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvPolicy {
    /// Extra parent variables to inherit when set (e.g. `SSH_AUTH_SOCK`).
    pub passthrough: Vec<String>,
    /// Variables to set explicitly; these override everything else.
    pub extra: BTreeMap<String, String>,
}

impl EnvPolicy {
    /// The complete child environment, sorted by name. Precedence: inherited
    /// allowlist and passthrough, then the fixed settings, then `extra`.
    pub fn build(&self) -> Vec<(String, String)> {
        let mut env = BTreeMap::new();
        let inherited = INHERITED
            .iter()
            .chain(INHERITED_WINDOWS)
            .copied()
            .chain(self.passthrough.iter().map(String::as_str));
        for key in inherited {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.to_string(), value);
            }
        }
        for (key, value) in FIXED {
            env.insert((*key).to_string(), (*value).to_string());
        }
        env.extend(self.extra.clone());
        env.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_fixed_and_extra_precedence() {
        let policy = EnvPolicy {
            passthrough: vec!["CARGO_PKG_NAME".into()],
            extra: BTreeMap::from([
                ("PAGER".into(), "less".into()),
                ("X_ONE".into(), "1".into()),
            ]),
        };
        let env: BTreeMap<String, String> = policy.build().into_iter().collect();
        assert!(env.contains_key("PATH"));
        assert_eq!(env.get("HYPERFORM").map(String::as_str), Some("1"));
        assert_eq!(
            env.get("GIT_TERMINAL_PROMPT").map(String::as_str),
            Some("0")
        );
        assert_eq!(env.get("PAGER").map(String::as_str), Some("less"));
        assert_eq!(env.get("X_ONE").map(String::as_str), Some("1"));
        assert_eq!(
            env.get("CARGO_PKG_NAME").map(String::as_str),
            Some("z-engine-host")
        );
        assert!(!env.contains_key("CARGO_MANIFEST_DIR"));
    }
}

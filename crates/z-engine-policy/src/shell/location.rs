//! Where a shell operand points, lexically.

use std::path::{Path, PathBuf};

use super::syntax::Word;
use crate::paths::{escapes_start, normalize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Location {
    /// Relative to the shell's working directory; `escapes` when it climbs
    /// above it at some point (`../x`).
    Relative {
        path: PathBuf,
        escapes: bool,
    },
    Absolute(PathBuf),
    /// Depends on run-time expansion (`$VAR`, `{a,b}`, `~user`, `cd -`).
    Unknown,
}

/// `~`, `~/x`, `$HOME/x` and `${HOME}/x` resolve through `home`; any other
/// expansion leaves the location unknown.
pub(crate) fn locate(word: &Word, home: Option<&Path>) -> Location {
    let text = word.text.as_str();
    if word.brace || text.is_empty() || (word.glob && glob_may_climb(text)) {
        return Location::Unknown;
    }
    if word.tilde || word.param {
        return match (home_suffix(word), home) {
            (Some(rest), Some(home)) => Location::Absolute(normalize(&home.join(rest))),
            _ => Location::Unknown,
        };
    }
    let path = Path::new(text);
    if path.is_absolute() {
        return Location::Absolute(normalize(path));
    }
    Location::Relative {
        path: path.to_path_buf(),
        escapes: escapes_start(path),
    }
}

/// `x` for `~/x`, `$HOME/x` or `${HOME}/x`, empty for the home itself;
/// `None` for `~user` or when anything else expands.
fn home_suffix(word: &Word) -> Option<&str> {
    let text = word.text.as_str();
    let rest = if word.tilde {
        text.strip_prefix('~')?
    } else {
        text.strip_prefix("${HOME}").or_else(|| {
            text.strip_prefix("$HOME")
                .filter(|rest| !rest.starts_with(|c: char| c == '_' || c.is_ascii_alphanumeric()))
        })?
    };
    if rest.contains('$') {
        return None;
    }
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix('/')
        .map(|rest| rest.trim_start_matches('/'))
}

/// A glob component starting with `.` may match `..` in older shells.
fn glob_may_climb(text: &str) -> bool {
    text.split('/')
        .any(|part| part.starts_with('.') && part.contains(['*', '?', '[']))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::lexer::lex;

    fn at(command: &str, home: Option<&Path>) -> Location {
        let lexed = lex(command);
        locate(&lexed.segments[0].command()[1], home)
    }

    #[test]
    fn paths_are_classified() {
        let home = Some(Path::new("/home/me"));
        let relative = |path: &str, escapes| Location::Relative {
            path: path.into(),
            escapes,
        };
        assert_eq!(at("cat src/a.rs", home), relative("src/a.rs", false));
        assert_eq!(at("cat ../a.rs", home), relative("../a.rs", true));
        assert_eq!(at("cat src/*.rs", home), relative("src/*.rs", false));
        assert_eq!(at("cat '$HOME'", home), relative("$HOME", false));
        assert_eq!(
            at("cat /etc/../etc/hosts", home),
            Location::Absolute("/etc/hosts".into())
        );
    }

    #[test]
    fn home_forms_resolve_through_the_home_directory() {
        let home = Some(Path::new("/home/me"));
        let absolute = |path: &str| Location::Absolute(path.into());
        assert_eq!(at("cat ~", home), absolute("/home/me"));
        assert_eq!(at("cat ~/.ssh/id", home), absolute("/home/me/.ssh/id"));
        assert_eq!(at("cat $HOME/.ssh/id", home), absolute("/home/me/.ssh/id"));
        assert_eq!(
            at("cat \"${HOME}/.zshrc\"", home),
            absolute("/home/me/.zshrc")
        );
        assert_eq!(at("cat $HOME", home), absolute("/home/me"));
        assert_eq!(at("cat ~/x", None), Location::Unknown);
    }

    #[test]
    fn other_expansions_are_unknown() {
        let home = Some(Path::new("/home/me"));
        for command in [
            "cat ~root/x",
            "cat $HOMEDIR/x",
            "cat $HOME/$FILE",
            "cat $DIR/x",
            "cat {a,b}",
            "cat .*/x",
            "cat ~/.*",
        ] {
            assert_eq!(at(command, home), Location::Unknown, "{command}");
        }
    }
}

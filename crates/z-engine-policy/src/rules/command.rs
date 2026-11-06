//! `Bash(..)` specifiers: exact commands and `:*` word prefixes.

use std::fmt;

use globset::GlobBuilder;

use crate::shell::{Word, parse};

/// `git status` (exact) or `npm test:*` (prefix); v1's `cargo test*` is read
/// as a prefix. Prefixes compare whole words after quote removal, so
/// `npm test:*` matches `npm test --watch` and `npm "test"` but not
/// `npm testing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandPattern {
    text: String,
    /// The words of `text` when it is one simple command without
    /// substitutions or redirects; other exact rules only match verbatim.
    words: Option<Vec<String>>,
    prefix: bool,
}

impl CommandPattern {
    /// `Ok(None)` for `*` and `:*`, which match every command like `Bash`.
    pub fn parse(spec: &str) -> Result<Option<Self>, String> {
        let spec = spec.trim();
        let (text, prefix) = match spec.strip_suffix(":*").or_else(|| spec.strip_suffix('*')) {
            Some(body) => (body.trim_end(), true),
            None => (spec, false),
        };
        if text.is_empty() {
            return if prefix {
                Ok(None)
            } else {
                Err("empty command".into())
            };
        }
        let parsed = parse(text);
        let simple = parsed.ok
            && !parsed.dynamic
            && parsed.segments.len() == 1
            && parsed.segments[0].redirects.is_empty();
        let words = simple
            .then(|| parsed.segments[0].argv())
            .filter(|words| !words.is_empty());
        if prefix && words.is_none() {
            return Err("a prefix rule must name one simple command".into());
        }
        Ok(Some(Self {
            text: text.to_string(),
            words,
            prefix,
        }))
    }

    /// Allow semantics for one simple command's words.
    pub fn allows(&self, argv: &[String]) -> bool {
        let Some(words) = &self.words else {
            return false;
        };
        if self.prefix {
            argv.starts_with(words)
        } else {
            argv == words.as_slice()
        }
    }

    /// Deny and ask semantics: like [`Self::allows`], but the program name
    /// ignores case (macOS and Windows resolve `RM` to `rm`) and a program
    /// word with wildcards or braces (`/bin/r?`, `{rm,x}`) counts when it
    /// could expand to the name.
    pub fn restricts(&self, command: &[Word]) -> bool {
        let Some((name, rest)) = self.words.as_deref().and_then(<[String]>::split_first) else {
            return false;
        };
        let Some((program, args)) = command.split_first() else {
            return false;
        };
        let args: Vec<&str> = args.iter().map(|word| word.text.as_str()).collect();
        let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
        let args_match = if self.prefix {
            args.starts_with(&rest)
        } else {
            args == rest
        };
        args_match && may_name(program, name)
    }

    /// Exact rules also match the whole command line verbatim.
    pub fn matches_line(&self, command: &str) -> bool {
        !self.prefix && command.trim() == self.text
    }
}

/// `program` is `name`, or a wildcard that could expand to it. A wildcard
/// that does not compile as a glob is assumed to match.
fn may_name(program: &Word, name: &str) -> bool {
    if program.text.eq_ignore_ascii_case(name) {
        return true;
    }
    if !(program.glob || program.brace) {
        return false;
    }
    GlobBuilder::new(&program.text)
        .case_insensitive(true)
        .build()
        .map_or(true, |glob| glob.compile_matcher().is_match(name))
}

impl fmt::Display for CommandPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.prefix {
            write!(f, "{}:*", self.text)
        } else {
            f.write_str(&self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(spec: &str) -> CommandPattern {
        CommandPattern::parse(spec).unwrap().unwrap()
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn prefixes_compare_whole_words() {
        let npm = pattern("npm test:*");
        assert!(npm.allows(&argv(&["npm", "test"])));
        assert!(npm.allows(&argv(&["npm", "test", "--watch"])));
        assert!(!npm.allows(&argv(&["npm", "testing"])));
        assert!(!npm.allows(&argv(&["npm"])));
        let legacy = pattern("cargo test*");
        assert_eq!(legacy.to_string(), "cargo test:*");
        assert!(legacy.allows(&argv(&["cargo", "test", "--lib"])));
    }

    #[test]
    fn exact_rules_match_words_or_the_verbatim_line() {
        let status = pattern("git status");
        assert!(status.allows(&argv(&["git", "status"])));
        assert!(!status.allows(&argv(&["git", "status", "-s"])));
        let compound = pattern("make build && make test");
        assert!(!compound.allows(&argv(&["make", "build"])));
        assert!(compound.matches_line("  make build && make test "));
        assert!(!pattern("npm test:*").matches_line("npm test"));
    }

    fn restricts(pattern: &CommandPattern, command: &str) -> bool {
        let parsed = parse(command);
        pattern.restricts(parsed.segments[0].command())
    }

    #[test]
    fn restrictive_matching_ignores_case_and_expands_wildcards() {
        let rm = pattern("rm:*");
        assert!(restricts(&rm, "RM -rf /"));
        assert!(!rm.allows(&argv(&["RM", "-rf", "/"])));
        assert!(restricts(&rm, "r? -rf /"));
        assert!(restricts(&rm, "{rm,x} -rf /"));
        assert!(restricts(&rm, "[r]m x"));
        assert!(!restricts(&rm, "l? -la"));
        assert!(!restricts(&rm, "'r?' x"));
        assert!(restricts(&pattern("git push:*"), "git push -f"));
        assert!(!restricts(&pattern("git push"), "git push -f"));
    }

    #[test]
    fn wildcards_and_invalid_prefixes() {
        assert_eq!(CommandPattern::parse("*"), Ok(None));
        assert_eq!(CommandPattern::parse(":*"), Ok(None));
        assert!(CommandPattern::parse("a && b:*").is_err());
        assert!(CommandPattern::parse("echo $(id):*").is_err());
        assert!(CommandPattern::parse("then:*").is_err());
        assert!(CommandPattern::parse("  ").is_err());
    }
}

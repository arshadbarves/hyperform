//! Command prefixes suggested for "always allow" rules.

use super::analysis::parse;
use super::read_only::segment_is_read_only;
use super::syntax::Word;

/// Programs where any prefix rule would grant too much.
const NEVER_PREFIX: &[&str] = &[
    "rm", "rmdir", "sudo", "doas", "su", "chmod", "chown", "chgrp", "dd", "shred", "mkfs", "kill",
    "killall", "pkill", "eval", "exec", "xargs", "env", "nohup", "command", "builtin", "source",
    ".", "time", "timeout", "nice", "export", "unset", "set", "alias", "trap",
];
/// Programs that must be followed by a script (or `-m module`) to be scoped.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish"];
const INTERPRETERS: &[&str] = &["python", "python3", "node", "ruby", "perl", "php"];
/// Subcommands that take the real target next (`npm run test`).
const RUN_VERBS: &[&str] = &["run", "run-script", "exec", "x", "dlx"];
/// Programs where the name alone would cover every subcommand.
const MULTIPLEXERS: &[&str] = &[
    "git", "cargo", "npm", "pnpm", "yarn", "bun", "deno", "go", "docker", "podman", "kubectl",
    "gh", "uv", "pip", "pip3", "poetry", "brew", "apt", "apt-get", "dotnet", "rustup", "npx",
];

/// The prefix for a `Bash(<prefix>:*)` rule covering `command`: the first
/// command that is not read-only, cut to its program and subcommand
/// (`cargo test -p x` gives `cargo test`, `npm run test` gives `npm run test`,
/// `python3 -m pytest -x` gives `python3 -m pytest`). `None` for dynamic or
/// unparsable commands and where any prefix would be too broad (`rm`, `sudo`,
/// `bash -c`, `python -c`, `git -C dir ..`).
pub fn suggest_prefix(command: &str) -> Option<String> {
    let parsed = parse(command);
    if !parsed.ok || parsed.dynamic {
        return None;
    }
    let segment = parsed
        .segments
        .iter()
        .find(|segment| !segment_is_read_only(segment))
        .or_else(|| parsed.segments.first())?;
    prefix_for(segment.command())
}

/// The prefix for one command; `words` starts at the command name (leading
/// assignments are kept, since allow rules compare them literally).
pub(crate) fn prefix_for(words: &[Word]) -> Option<String> {
    let assignments = words.iter().take_while(|word| word.assignment).count();
    let (env, rest) = words.split_at(assignments);
    let (name, args) = rest.split_first()?;
    let program = name.text.as_str();
    if name.expands() || program.is_empty() || NEVER_PREFIX.contains(&program) {
        return None;
    }
    let plain =
        |word: &&Word| !word.text.is_empty() && !word.text.starts_with('-') && !word.expands();
    let mut picked = vec![name];
    if SHELLS.contains(&program) || INTERPRETERS.contains(&program) {
        match args.first() {
            Some(flag) if flag.text == "-m" && INTERPRETERS.contains(&program) => {
                picked.push(flag);
                picked.push(args.get(1).filter(plain)?);
            }
            Some(script) if plain(&script) => picked.push(script),
            _ => return None,
        }
    } else if let Some(sub) = args.first().filter(plain) {
        picked.push(sub);
        let chained = RUN_VERBS.contains(&sub.text.as_str()) || sub.text.starts_with('+');
        if let Some(next) = args.get(1).filter(plain).filter(|_| chained) {
            picked.push(next);
        }
    } else if !args.is_empty() && MULTIPLEXERS.contains(&program) {
        return None;
    }
    let tokens: Vec<String> = env
        .iter()
        .chain(picked)
        .map(|word| quote(&word.text))
        .collect();
    Some(tokens.join(" "))
}

/// Quotes `text` for a rule so it lexes back to the same word.
fn quote(text: &str) -> String {
    let bare = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_./:=+@%,-".contains(c));
    if bare && !text.is_empty() {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_round_trips_through_the_lexer() {
        for text in ["plain", "a b", "it's", "*.rs", "$HOME", "a)"] {
            let quoted = quote(text);
            let lexed = crate::shell::lexer::lex(&format!("x {quoted}"));
            assert_eq!(lexed.segments[0].argv()[1], text, "{quoted}");
        }
    }
}

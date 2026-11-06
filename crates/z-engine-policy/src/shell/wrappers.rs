//! Commands a segment may run indirectly. Deny and ask rules check every
//! candidate, so a rule for `rm` also catches `sudo rm`, `/bin/rm`,
//! `env X=1 rm`, `rm${IFS}-rf`, `bash -c 'rm ..'`, `eval rm ..` and
//! `find -exec rm`. The expansion over-approximates on purpose: it only
//! widens restrictive matches and is never used to allow anything.

use super::fields::expansion_readings;
use super::lexer::lex;
use super::syntax::{Segment, Word};

/// Programs that run the command given in their later arguments.
#[rustfmt::skip]
const WRAPPERS: &[&str] = &[
    "sudo", "doas", "pkexec", "runuser", "env", "nohup", "time", "command", "builtin", "exec",
    "nice", "ionice", "renice", "stdbuf", "timeout", "gtimeout", "xargs", "caffeinate", "chrt",
    "taskset", "setsid", "unbuffer", "flock", "busybox", "watch", "strace", "ltrace", "nsenter",
    "chroot", "parallel",
];
/// Shells whose `-c` argument is a command line.
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "mksh", "ash", "fish", "su",
];
const FIND_ACTIONS: &[&str] = &["-exec", "-execdir", "-ok", "-okdir"];
const MAX_CANDIDATES: usize = 256;
const MAX_DEPTH: usize = 8;

/// Candidate commands for one segment, the literal command first. Each
/// candidate's words start at its program name. The flag reports that the
/// expansion hit its budget, in which case callers must fail closed.
pub(crate) fn executed_commands(segment: &Segment) -> (Vec<Segment>, bool) {
    let start = Segment {
        words: segment.command().to_vec(),
        redirects: segment.redirects.clone(),
    };
    let mut found: Vec<Segment> = Vec::new();
    let mut queue = vec![(start, 0usize)];
    let mut truncated = false;
    while let Some((candidate, depth)) = queue.pop() {
        if found.contains(&candidate) {
            continue;
        }
        if found.len() >= MAX_CANDIDATES {
            return (found, true);
        }
        let next = expand(&candidate);
        if depth >= MAX_DEPTH {
            truncated |= !next.is_empty();
        } else {
            queue.extend(next.into_iter().map(|segment| (segment, depth + 1)));
        }
        found.push(candidate);
    }
    (found, truncated)
}

fn expand(candidate: &Segment) -> Vec<Segment> {
    let words = &candidate.words;
    let Some(first) = words.first() else {
        return Vec::new();
    };
    let with = |words: &[Word]| Segment {
        words: words.to_vec(),
        redirects: candidate.redirects.clone(),
    };
    let mut out: Vec<Segment> = expansion_readings(words)
        .into_iter()
        .flatten()
        .filter(|reading| !reading.is_empty())
        .map(|reading| with(&reading))
        .collect();
    let assignments = words.iter().take_while(|word| word.assignment).count();
    if assignments > 0 {
        out.push(with(&words[assignments..]));
        return out;
    }
    let base = first.text.rsplit('/').next().unwrap_or_default();
    if base != first.text && !base.is_empty() {
        let mut renamed = words.clone();
        renamed[0].text = base.to_string();
        out.push(with(&renamed));
        return out;
    }
    let name = base.to_ascii_lowercase();
    if WRAPPERS.contains(&name.as_str()) {
        for (at, word) in words.iter().enumerate().skip(1) {
            if !word.text.starts_with('-') {
                out.push(with(&words[at..]));
            }
        }
    }
    let script = match name.as_str() {
        _ if SHELLS.contains(&name.as_str()) => shell_script(words),
        "eval" | "watch" => Some(join(&words[1..])),
        "env" => split_string(words),
        _ => None,
    };
    out.extend(
        script
            .map(|script| script_commands(&script))
            .unwrap_or_default(),
    );
    if name == "find" {
        out.extend(find_actions(words).iter().map(|words| with(words)));
    }
    out
}

/// The command line after a `-c` option cluster (`-c`, `-lc`, `--command=`).
fn shell_script(words: &[Word]) -> Option<String> {
    let mut rest = words.iter().skip(1);
    while let Some(word) = rest.next() {
        let text = word.text.as_str();
        if let Some(script) = text.strip_prefix("--command=") {
            return Some(script.to_string());
        }
        let cluster = text
            .strip_prefix('-')
            .filter(|flags| !flags.starts_with('-'));
        if text == "--command" || cluster.is_some_and(|flags| flags.contains('c')) {
            return rest
                .find(|next| !next.text.starts_with('-'))
                .map(|next| next.text.clone());
        }
    }
    None
}

/// `env -S 'cmd args'` and `env --split-string=..`.
fn split_string(words: &[Word]) -> Option<String> {
    let mut rest = words.iter().skip(1);
    while let Some(word) = rest.next() {
        let text = word.text.as_str();
        if text == "-S" || text == "--split-string" {
            return rest.next().map(|next| next.text.clone());
        }
        if let Some(value) = text.strip_prefix("--split-string=") {
            return Some(value.to_string());
        }
        if let Some(value) = text.strip_prefix("-S").filter(|v| !v.is_empty()) {
            return Some(value.to_string());
        }
    }
    None
}

/// Commands after `find -exec` and friends, up to `;` or `+`.
fn find_actions(words: &[Word]) -> Vec<Vec<Word>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < words.len() {
        if FIND_ACTIONS.contains(&words[at].text.as_str()) {
            let command: Vec<Word> = words[at + 1..]
                .iter()
                .take_while(|word| word.text != ";" && word.text != "+")
                .cloned()
                .collect();
            at += command.len();
            if !command.is_empty() {
                out.push(command);
            }
        }
        at += 1;
    }
    out
}

fn script_commands(script: &str) -> Vec<Segment> {
    let lexed = lex(script);
    lexed
        .segments
        .into_iter()
        .chain(lexed.nested)
        .map(|segment| Segment {
            words: segment.command().to_vec(),
            redirects: segment.redirects,
        })
        .collect()
}

fn join(words: &[Word]) -> String {
    let texts: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    texts.join(" ")
}

#[cfg(test)]
#[path = "wrappers_tests.rs"]
mod tests;

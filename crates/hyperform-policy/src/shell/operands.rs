//! Paths a shell command names as operands. Best effort: programs can open
//! files that never appear in their arguments. Used to keep auto-allowed
//! read-only commands inside allowed directories, to confine acceptEdits
//! filesystem commands, and to apply `Read(..)` and `Edit(..)` deny and ask
//! rules to shell commands.

use super::sed;
use super::syntax::Word;

/// Programs whose arguments are not file names.
const NO_FILES: &[&str] = &[
    "echo", "printf", "pwd", "true", "false", "seq", "expr", "date", "id", "whoami", "hostname",
    "uname", "env", "printenv", "getconf", "basename", "dirname", "type", "which", "test", "[",
    "nproc", "groups", "uptime", "numfmt",
];
/// Programs whose first positional argument is a pattern or filter.
const PATTERN_FIRST: &[&str] = &["grep", "egrep", "fgrep", "rg", "jq"];
const PATTERN_FLAGS: &[&str] = &["-e", "--regexp", "-f", "--file", "--from-file", "--files"];
/// Programs that create, change, or remove their operands.
const WRITERS: &[&str] = &[
    "rm", "rmdir", "touch", "mkdir", "ln", "truncate", "chmod", "chown", "chgrp", "tee", "shred",
    "unlink",
];
/// Programs that read some operands and write others.
const COPIERS: &[&str] = &["cp", "mv", "install", "rsync"];

#[derive(Debug, Default)]
pub(crate) struct Operands {
    pub reads: Vec<Word>,
    pub writes: Vec<Word>,
}

/// Operands of one command (`words` starts at the program name).
pub(crate) fn operands(words: &[Word]) -> Operands {
    let Some((name, args)) = words.split_first() else {
        return Operands::default();
    };
    let name = name.text.as_str();
    let reads = |reads: Vec<Word>| Operands {
        reads,
        writes: Vec::new(),
    };
    match name {
        _ if NO_FILES.contains(&name) => Operands::default(),
        "cd" => reads(vec![cd_target(args)]),
        "find" => reads(
            args.iter()
                .take_while(|word| !starts_find_expression(&word.text))
                .cloned()
                .collect(),
        ),
        "sed" => match sed::parse_call(args) {
            Some(call) if call.in_place => Operands {
                reads: Vec::new(),
                writes: call.files.into_iter().cloned().collect(),
            },
            Some(call) => reads(call.files.into_iter().cloned().collect()),
            None => reads(positionals(args)),
        },
        "dd" => Operands {
            reads: dd_operands(args, "if="),
            writes: dd_operands(args, "of="),
        },
        _ if PATTERN_FIRST.contains(&name) => {
            let mut files = positionals(args);
            let explicit = args
                .iter()
                .any(|word| PATTERN_FLAGS.iter().any(|flag| word.text.starts_with(flag)));
            if !explicit && !files.is_empty() {
                files.remove(0);
            }
            reads(files)
        }
        _ if COPIERS.contains(&name) => {
            let files = positionals(args);
            Operands {
                reads: files.clone(),
                writes: files,
            }
        }
        _ if WRITERS.contains(&name) => Operands {
            reads: Vec::new(),
            writes: positionals(args),
        },
        _ => reads(positionals(args)),
    }
}

/// Non-option arguments plus the values of `--flag=value` options. URLs are
/// skipped because they are not local paths.
fn positionals(args: &[Word]) -> Vec<Word> {
    let mut out = Vec::new();
    let mut options = true;
    for word in args {
        let text = word.text.as_str();
        if options && text == "--" {
            options = false;
        } else if options && text.starts_with('-') && text.len() > 1 {
            if let Some((_, value)) = text.split_once('=').filter(|_| text.starts_with("--")) {
                if !value.is_empty() {
                    out.push(Word {
                        text: value.to_string(),
                        tilde: false,
                        ..word.clone()
                    });
                }
            }
        } else if !text.contains("://") {
            out.push(word.clone());
        }
    }
    out
}

/// `cd` with no directory goes home; `cd -` goes somewhere unknown.
fn cd_target(args: &[Word]) -> Word {
    let target = args
        .iter()
        .find(|word| !matches!(word.text.as_str(), "-L" | "-P" | "-e" | "-@"));
    match target {
        None => Word {
            text: "~".to_string(),
            tilde: true,
            ..Word::default()
        },
        Some(word) if word.text == "-" => Word {
            text: "-".to_string(),
            param: true,
            ..Word::default()
        },
        Some(word) => word.clone(),
    }
}

fn starts_find_expression(text: &str) -> bool {
    (text.starts_with('-') && text.len() > 1) || matches!(text, "(" | ")" | "!" | ",")
}

fn dd_operands(args: &[Word], key: &str) -> Vec<Word> {
    args.iter()
        .filter_map(|word| {
            word.text.strip_prefix(key).map(|value| Word {
                text: value.to_string(),
                tilde: false,
                ..word.clone()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::lexer::lex;

    fn words(command: &str) -> Vec<Word> {
        lex(command).segments[0].command().to_vec()
    }

    fn texts(words: &[Word]) -> Vec<&str> {
        words.iter().map(|word| word.text.as_str()).collect()
    }

    #[test]
    fn readers_name_their_files() {
        let ops = operands(&words("cat -n --tabsize=4 a.txt /etc/hosts -- -dash"));
        assert_eq!(texts(&ops.reads), ["4", "a.txt", "/etc/hosts", "-dash"]);
        assert!(ops.writes.is_empty());
        assert!(operands(&words("echo /etc/passwd")).reads.is_empty());
        assert_eq!(
            texts(&operands(&words("curl https://x.io/a")).reads),
            [] as [&str; 0]
        );
    }

    #[test]
    fn pattern_first_programs_skip_the_pattern() {
        assert_eq!(texts(&operands(&words("grep -rn /api src")).reads), ["src"]);
        assert_eq!(
            texts(&operands(&words("grep -e /api src ../x")).reads),
            ["/api", "src", "../x"]
        );
        assert_eq!(
            texts(&operands(&words("jq .name ~/pkg.json")).reads),
            ["~/pkg.json"]
        );
    }

    #[test]
    fn find_cd_sed_and_dd() {
        assert_eq!(
            texts(&operands(&words("find /tmp src -name '*.rs'")).reads),
            ["/tmp", "src"]
        );
        assert_eq!(texts(&operands(&words("cd")).reads), ["~"]);
        assert!(operands(&words("cd -")).reads[0].param);
        assert_eq!(texts(&operands(&words("cd -P ../x")).reads), ["../x"]);
        assert_eq!(texts(&operands(&words("sed -n 1p a b")).reads), ["a", "b"]);
        assert_eq!(texts(&operands(&words("sed -i s/a/b/ f")).writes), ["f"]);
        let dd = operands(&words("dd if=/dev/zero of=disk.img bs=1M"));
        assert_eq!(
            (texts(&dd.reads), texts(&dd.writes)),
            (vec!["/dev/zero"], vec!["disk.img"])
        );
    }

    #[test]
    fn writers_and_copiers() {
        let ops = operands(&words("rm -rf build dist"));
        assert_eq!(texts(&ops.writes), ["build", "dist"]);
        let ops = operands(&words("cp .env /tmp/leak"));
        assert_eq!(texts(&ops.reads), [".env", "/tmp/leak"]);
        assert_eq!(texts(&ops.writes), [".env", "/tmp/leak"]);
    }
}

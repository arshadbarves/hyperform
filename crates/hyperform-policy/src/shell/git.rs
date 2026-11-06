//! Read-only `git` invocations: inspection and listing subcommands with flags
//! that cannot write files, run external programs, or change refs.

use super::syntax::Word;

/// Subcommands that only inspect the repository.
const INSPECT: &[&str] = &[
    "status",
    "diff",
    "log",
    "show",
    "rev-parse",
    "ls-files",
    "blame",
    "describe",
    "ls-tree",
    "cat-file",
    "rev-list",
    "merge-base",
    "show-ref",
    "shortlog",
    "grep",
];
/// Options that make an inspecting subcommand write a file or run a program.
const FORBIDDEN: &[&str] = &[
    "--output",
    "--ext-diff",
    "--open-files-in-pager",
    "--textconv",
    "--filters",
];
const BRANCH_FLAGS: &[&str] = &[
    "--all",
    "--remotes",
    "--list",
    "--verbose",
    "--show-current",
    "--no-color",
    "--color",
    "--column",
    "--no-column",
    "--ignore-case",
    "--omit-empty",
    "--no-abbrev",
    "--quiet",
];
const TAG_FLAGS: &[&str] = &[
    "--list",
    "--column",
    "--no-column",
    "--ignore-case",
    "--color",
];
/// Filters taking an optional commit; they imply listing.
const LIST_FILTERS: &[&str] = &[
    "--contains",
    "--no-contains",
    "--merged",
    "--no-merged",
    "--points-at",
];
const CONFIG_READS: &[&str] = &[
    "--get",
    "--get-all",
    "--get-regexp",
    "--get-urlmatch",
    "--get-color",
    "--get-colorbool",
    "--list",
    "-l",
];
const CONFIG_WRITES: &[&str] = &[
    "--add",
    "--unset",
    "--unset-all",
    "--replace-all",
    "--rename-section",
    "--remove-section",
    "--edit",
    "-e",
    "set",
    "unset",
    "rename-section",
    "remove-section",
    "edit",
];

pub(crate) fn is_read_only(args: &[Word]) -> bool {
    if args.iter().any(Word::splits) {
        return false;
    }
    let mut rest = args;
    loop {
        rest = match rest {
            [flag, tail @ ..]
                if matches!(
                    flag.text.as_str(),
                    "--no-pager" | "-P" | "--no-optional-locks" | "--literal-pathspecs"
                ) =>
            {
                tail
            }
            [flag, _dir, tail @ ..] if flag.text == "-C" => tail,
            _ => break,
        };
    }
    let Some((subcommand, args)) = rest.split_first() else {
        return false;
    };
    let texts: Vec<&str> = args.iter().map(|word| word.text.as_str()).collect();
    match subcommand.text.as_str() {
        "--version" | "version" => true,
        "grep" if texts.iter().any(|t| t.starts_with("-O")) => false,
        name if INSPECT.contains(&name) => !texts.iter().any(|text| forbidden(text)),
        "branch" => lists_only(&texts, BRANCH_FLAGS, "arlviq"),
        "tag" => lists_only(&texts, TAG_FLAGS, "li"),
        "remote" => {
            matches!(texts.as_slice(), [] | ["-v" | "--verbose"])
                || texts.first() == Some(&"get-url")
        }
        "stash" => matches!(texts.first(), Some(&("list" | "show"))),
        "worktree" => texts.first() == Some(&"list"),
        "config" => config_reads_only(&texts),
        _ => false,
    }
}

fn forbidden(text: &str) -> bool {
    FORBIDDEN
        .iter()
        .any(|flag| text == *flag || text.strip_prefix(flag).is_some_and(|v| v.starts_with('=')))
}

/// `git branch` / `git tag` in listing mode: known flags only, and names
/// (which would create refs) only as patterns after `--list` or a filter.
fn lists_only(args: &[&str], flags: &[&str], short: &str) -> bool {
    let mut listing = false;
    let mut names = 0;
    let mut at = 0;
    while at < args.len() {
        let text = args[at];
        let (flag, has_value) = match text.split_once('=') {
            Some((flag, _)) => (flag, true),
            None => (text, false),
        };
        if LIST_FILTERS.contains(&flag) {
            listing = true;
            if !has_value && args.get(at + 1).is_some_and(|next| !next.starts_with('-')) {
                at += 1;
            }
        } else if matches!(flag, "--sort" | "--format") {
            if !has_value {
                at += 1;
            }
        } else if flags.contains(&flag) || flag.starts_with("--abbrev") {
            listing |= flag == "--list";
        } else if let Some(cluster) = text.strip_prefix('-') {
            let known = !cluster.is_empty()
                && cluster
                    .chars()
                    .all(|c| short.contains(c) || (c == 'n' && short == "li"))
                || (short == "li"
                    && cluster.starts_with('n')
                    && cluster[1..].chars().all(|c| c.is_ascii_digit()));
            if !known {
                return false;
            }
            listing |= cluster.contains('l');
        } else {
            names += 1;
        }
        at += 1;
    }
    names == 0 || listing
}

fn config_reads_only(args: &[&str]) -> bool {
    if args.iter().any(|text| CONFIG_WRITES.contains(text)) {
        return false;
    }
    if args.iter().any(|text| CONFIG_READS.contains(text))
        || matches!(args.first(), Some(&("get" | "list")))
    {
        return true;
    }
    let scope = [
        "--global",
        "--local",
        "--system",
        "--worktree",
        "--show-origin",
        "--show-scope",
    ];
    let names = args.iter().filter(|text| !text.starts_with('-')).count();
    names == 1
        && args
            .iter()
            .all(|text| !text.starts_with('-') || scope.contains(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::lexer::lex;

    fn read_only(command: &str) -> bool {
        let lexed = lex(command);
        let words = lexed.segments[0].command();
        assert_eq!(words[0].text, "git");
        is_read_only(&words[1..])
    }

    #[test]
    fn inspection_is_read_only() {
        for command in [
            "git status --porcelain",
            "git diff HEAD~1 -- src",
            "git log --oneline -n 5",
            "git show abc:src/lib.rs",
            "git blame -L 1,20 main.rs",
            "git rev-parse --show-toplevel",
            "git ls-files -m",
            "git grep -n TODO",
            "git --no-pager log -p",
            "git -C sub status",
            "git --version",
            "git branch",
            "git branch -a -v",
            "git branch -vv",
            "git branch --show-current",
            "git branch --list 'feat*'",
            "git branch --merged main",
            "git branch --contains abc123",
            "git tag",
            "git tag -l 'v1.*'",
            "git tag -n5",
            "git remote -v",
            "git remote get-url origin",
            "git stash list",
            "git worktree list",
            "git config --get user.name",
            "git config user.email",
            "git config --global --list",
        ] {
            assert!(read_only(command), "{command} should be read-only");
        }
    }

    #[test]
    fn writes_ref_changes_and_program_runs_are_not() {
        for command in [
            "git",
            "git push",
            "git commit -m x",
            "git checkout -b feat",
            "git clean -fd",
            "git stash",
            "git branch -D x",
            "git branch new-feature",
            "git branch -m old new",
            "git tag v1.0",
            "git tag -d v1.0",
            "git remote add origin url",
            "git diff --output=patch.txt",
            "git log --output patch.txt",
            "git diff --ext-diff",
            "git grep -Oless foo",
            "git cat-file --textconv HEAD:a",
            "git -c core.pager=evil log",
            "git --git-dir=/tmp/x status",
            "git config user.name evil",
            "git config --unset user.name",
            "git config --add x.y z",
            "git help log",
            "git log $ARGS",
            "git diff *",
        ] {
            assert!(!read_only(command), "{command} should not be read-only");
        }
    }
}

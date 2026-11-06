//! Read-only shell commands: a first-word allowlist of programs that only
//! print (ported from v1's `SAFE_BASH`), with per-program guards for flags
//! that write, delete, or run other programs. Unknown programs, path-qualified
//! programs, and assignments are never read-only.

use super::analysis::{Parsed, parse};
use super::syntax::{Redirect, Segment, Word};
use super::{git, sed};

/// Programs that only print, whatever their arguments.
const SAFE: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "grep",
    "egrep",
    "fgrep",
    "wc",
    "which",
    "type",
    "stat",
    "du",
    "df",
    "pwd",
    "echo",
    "cd",
    "diff",
    "cmp",
    "comm",
    "printenv",
    "id",
    "whoami",
    "uname",
    "seq",
    "expr",
    "test",
    "[",
    "fmt",
    "pr",
    "numfmt",
    "tsort",
    "getconf",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "md5sum",
    "shasum",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "b2sum",
    "cksum",
    "nl",
    "jq",
    "true",
    "false",
    "cut",
    "tr",
    "paste",
    "rev",
    "tac",
    "column",
    "strings",
    "od",
    "hexdump",
    "nproc",
    "groups",
    "uptime",
];
/// Programs with dangerous flags: an argument that expands at run time
/// (`*`, `{a,b}`, `$x`) could smuggle one in, e.g. a file named `--pre=sh`.
const FLAG_SENSITIVE: &[&str] = &[
    "find", "sort", "uniq", "sed", "git", "env", "date", "hostname", "tree", "file", "rg", "bat",
    "printf",
];
const FIND_ACTIONS: &[&str] = &[
    "-delete", "-exec", "-execdir", "-ok", "-okdir", "-fprint", "-fprint0", "-fprintf", "-fls",
];
/// Toolchains whose `--version` / `-V` only print.
const VERSION_PROBES: &[&str] = &[
    "cargo", "rustc", "rustup", "node", "npm", "pnpm", "yarn", "bun", "deno", "python", "python3",
    "go", "uv", "pip", "pip3", "ruby", "java", "gcc", "clang", "make", "cmake",
];
/// Toolchains where `-v` also means `--version`.
const DASH_V_PROBES: &[&str] = &["node", "npm", "pnpm", "yarn", "bun"];
const HOSTNAME_FLAGS: &[&str] = &[
    "-f",
    "--fqdn",
    "--long",
    "-s",
    "--short",
    "-i",
    "--ip-address",
    "-I",
    "--all-ip-addresses",
    "-d",
    "--domain",
    "-A",
    "--all-fqdns",
    "-a",
    "--alias",
];
const BAT_WRITES: &[&str] = &[
    "--pager",
    "--paging",
    "--config-file",
    "--generate-config-file",
];

/// True when the command line only reads: every segment runs an allowlisted
/// program with safe flags, nothing redirects into a file, and nothing is
/// computed at run time (substitutions, `eval`). Reads outside the project
/// are still read-only; the policy checks locations separately.
pub fn is_read_only(command: &str) -> bool {
    command_is_read_only(&parse(command))
}

pub(crate) fn command_is_read_only(parsed: &Parsed) -> bool {
    parsed.ok
        && !parsed.dynamic
        && !parsed.segments.is_empty()
        && parsed.segments.iter().all(segment_is_read_only)
}

pub(crate) fn segment_is_read_only(segment: &Segment) -> bool {
    if segment.redirects.iter().any(Redirect::writes_file) {
        return false;
    }
    match segment.command().split_first() {
        None => true,
        Some((name, _)) if name.assignment || name.expands() => false,
        Some((name, args)) => program_is_read_only(&name.text, args),
    }
}

fn program_is_read_only(name: &str, args: &[Word]) -> bool {
    if FLAG_SENSITIVE.contains(&name) && args.iter().any(Word::splits) {
        return false;
    }
    let has = |pred: &dyn Fn(&str) -> bool| args.iter().any(|arg| pred(&arg.text));
    match name {
        "find" => !has(&|a| FIND_ACTIONS.contains(&a)),
        "sort" => !has(&|a| {
            short_flags(a).is_some_and(|f| f.contains('o'))
                || a.starts_with("--output")
                || a.starts_with("--compress-program")
        }),
        "uniq" => operand_count(args, &["-f", "-s", "-w"]) < 2,
        "sed" => sed::parse_call(args).is_some_and(|call| !call.in_place),
        "git" => git::is_read_only(args),
        "env" => env_only_prints(args),
        "date" => date_only_prints(args),
        "hostname" => args
            .iter()
            .all(|arg| HOSTNAME_FLAGS.contains(&arg.text.as_str())),
        "tree" => !has(&|a| short_flags(a).is_some_and(|f| f.contains('o'))),
        "file" => !has(&|a| short_flags(a).is_some_and(|f| f.contains('C')) || a == "--compile"),
        "rg" => {
            !has(&|a| a == "--pre" || a.starts_with("--pre=") || a.starts_with("--hostname-bin"))
        }
        "bat" => {
            args.first().is_none_or(|first| first.text != "cache")
                && !has(&|a| BAT_WRITES.iter().any(|flag| a.starts_with(flag)))
        }
        "printf" => args
            .first()
            .is_none_or(|first| !first.text.starts_with("-v")),
        "cargo" if args.len() == 1 && args[0].text == "--list" => true,
        _ if VERSION_PROBES.contains(&name) => version_probe(name, args),
        _ => SAFE.contains(&name),
    }
}

/// The letters of a short-option cluster (`-rn` gives `rn`).
fn short_flags(text: &str) -> Option<&str> {
    text.strip_prefix('-')
        .filter(|flags| !flags.is_empty() && !flags.starts_with('-'))
}

/// Non-option operands, skipping the values of `value_flags`.
fn operand_count(args: &[Word], value_flags: &[&str]) -> usize {
    let mut count = 0;
    let mut options = true;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.text.as_str();
        if options && text == "--" {
            options = false;
        } else if options && text.starts_with('-') && text.len() > 1 {
            if value_flags.contains(&text) {
                iter.next();
            }
        } else {
            count += 1;
        }
    }
    count
}

/// `env` with only assignments and printing flags; anything else runs a program.
fn env_only_prints(args: &[Word]) -> bool {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.text.as_str() {
            "-0" | "--null" | "-i" | "--ignore-environment" | "-" => {}
            "-u" | "--unset" => {
                if iter.next().is_none() {
                    return false;
                }
            }
            text if text.starts_with("--unset=") || (text.starts_with("-u") && text.len() > 2) => {}
            text if !text.starts_with('-') && text.contains('=') => {}
            _ => return false,
        }
    }
    true
}

/// `date` that prints: no `-s`/`--set`, and positional arguments only as
/// `+FORMAT` (a bare date operand sets the clock) unless BSD `-j` is given.
fn date_only_prints(args: &[Word]) -> bool {
    let mut no_set = false;
    let mut operands = 0;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.text.as_str();
        if text.starts_with('+') {
            continue;
        }
        if text == "--set" || text.starts_with("--set=") {
            return false;
        }
        if matches!(text, "--date" | "--reference" | "--file") {
            iter.next();
            continue;
        }
        if text.starts_with("--") {
            continue;
        }
        let Some(flags) = short_flags(text) else {
            operands += 1;
            continue;
        };
        for (index, flag) in flags.char_indices() {
            match flag {
                's' => return false,
                'j' => no_set = true,
                'd' | 'r' | 'f' | 'v' | 'z' => {
                    if index + 1 == flags.len() {
                        iter.next();
                    }
                    break;
                }
                'I' => break,
                _ => {}
            }
        }
    }
    operands == 0 || no_set
}

fn version_probe(name: &str, args: &[Word]) -> bool {
    !args.is_empty()
        && args.iter().all(|arg| {
            matches!(arg.text.as_str(), "--version" | "-V")
                || (arg.text == "-v" && DASH_V_PROBES.contains(&name))
        })
}

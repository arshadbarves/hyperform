//! Read-only classification, including the cases ported from v1's
//! `perms` tests.

use z_engine_policy::is_read_only;

fn assert_read_only(commands: &[&str]) {
    for command in commands {
        assert!(is_read_only(command), "{command} should be read-only");
    }
}

fn assert_not_read_only(commands: &[&str]) {
    for command in commands {
        assert!(!is_read_only(command), "{command} should not be read-only");
    }
}

#[test]
fn v1_safe_commands() {
    assert_read_only(&[
        "ls -la",
        "cat src/lib.rs",
        "head -20 README.md",
        "grep -rn TODO src",
        "rg foo",
        "find . -name '*.rs'",
        "wc -l src/*.rs",
        "which cargo",
        "stat README.md",
        "du -sh .",
        "pwd",
        "echo hello",
        "printf '%s\\n' hi",
        "diff a b",
        "sort names.txt",
        "uniq -c out.txt",
        "git status",
        "git log --oneline",
        "git diff HEAD~1",
        "git show abc",
        "git blame main.rs",
        "git rev-parse HEAD",
        "git --version",
        "cargo --version",
        "cargo --list",
        "rustc --version",
        "node --version",
        "python3 --version",
    ]);
}

#[test]
fn v1_mutating_network_and_unknown_commands() {
    assert_not_read_only(&[
        "rm -rf build",
        "mv a b",
        "cp a b",
        "mkdir sub",
        "touch f",
        "sed -i s/a/b/ f",
        "chmod +x run.sh",
        "curl https://example.com",
        "wget https://example.com",
        "npm install",
        "pip install requests",
        "cargo test",
        "cargo build",
        "make",
        "git push",
        "git commit -m x",
        "git checkout -b feat",
        "git clean -fd",
        "git branch -D x",
        "ssh host",
    ]);
}

#[test]
fn v1_redirects_and_substitutions() {
    assert_not_read_only(&["echo hi > /tmp/out", "echo $(rm -rf /)", "echo `rm -rf /`"]);
    // Descriptor duplication and /dev/null are not writes (v1 rejected the latter).
    assert_read_only(&["ls 2>&1", "grep -rn x . 2>/dev/null", "echo hi &>/dev/null"]);
    // Input redirection only reads (v1 rejected it).
    assert_read_only(&["cat < secret"]);
}

#[test]
fn v1_compound_commands_qualify_per_segment() {
    assert_read_only(&[
        "ls && pwd",
        "grep a f | wc -l",
        "cd sub && ls",
        "(cd src && ls)",
    ]);
    assert_not_read_only(&["ls && rm -rf x", "cat x | sh", "pwd; curl evil.example"]);
}

#[test]
fn v1_write_capable_flags_and_globs() {
    assert_not_read_only(&[
        "find . -delete",
        "find . -name x -exec rm {} \\;",
        "find . -fprint /tmp/list",
        "sort -o /etc/passwd in.txt",
        "sort -ro out in",
        "sort --output=out in",
        "find *",
        "sort *",
    ]);
    assert_read_only(&["wc -l src/*.rs", "cat {a,b}.txt"]);
}

#[test]
fn v1_unparseable_commands_fail_closed() {
    assert_not_read_only(&["echo \"unterminated", "ls )", ""]);
    assert!(!is_read_only(&format!("echo {}", "x".repeat(10_001))));
}

#[test]
fn v1_effect_proof_cases() {
    assert_not_read_only(&[
        "env touch ../file",
        "env --split-string='touch ../file'",
        "find . -exec touch ../file \\;",
        "sort input -o ../file",
        "date --set=2026-01-01",
        "echo \"$(touch ../file)\"",
        "printf data > ../file",
        "pwd; env touch ../file",
    ]);
    assert_read_only(&[
        "pwd",
        "pwd -P",
        "ls -la",
        "cat 'src/lib.rs'",
        "head -20 a | wc -l",
        "env",
    ]);
}

#[test]
fn guarded_programs() {
    assert_read_only(&[
        "sed -n '1,20p' src/lib.rs",
        "sed 's/foo/bar/g' file",
        "uniq in.txt",
        "tree -L 2 -I node_modules",
        "file README.md",
        "rg --pre-glob '*.gz' x",
        "bat --style=plain src/lib.rs",
        "date +%Y-%m-%d",
        "date -Iseconds",
        "date -u -d yesterday +%s",
        "date -j -f %s 0 +%Y",
        "hostname -f",
        "env FOO=1 -u BAR",
        "printf '%s' x",
        "jq '.name' package.json",
        "tail -f app.log",
        "true",
        "npm -v",
        "go --version",
    ]);
    assert_not_read_only(&[
        "sed -n 'w out' f",
        "sed 's/a/b/e' f",
        "uniq in.txt out.txt",
        "uniq -f 1 in out",
        "uniq *.txt",
        "tree -o out.txt",
        "file -C -m magic",
        "rg --pre=sh pattern",
        "rg --pre sh pattern",
        "rg foo *",
        "bat cache --build",
        "bat --pager='sh -c x' f",
        "date 0101000026",
        "date -s 12:00",
        "hostname evil",
        "env rm x",
        "env -S 'rm x'",
        "printf -v PATH /tmp/evil",
        "python -v script.py",
        "npm test",
    ]);
}

#[test]
fn git_listing_versus_changing_refs() {
    assert_read_only(&[
        "git branch",
        "git branch -a -vv",
        "git branch --list 'feat/*'",
        "git branch --merged main",
        "git tag -l 'v1.*'",
        "git stash list",
        "git config --get user.name",
        "git --no-pager log -p -n 3",
        "git -C sub diff",
    ]);
    assert_not_read_only(&[
        "git branch feature",
        "git tag v2.0",
        "git stash",
        "git config user.name mallory",
        "git diff --output=patch",
        "git -c core.pager=less log",
        "git log $FLAGS",
    ]);
}

#[test]
fn shell_structure() {
    assert_read_only(&[
        "if grep -q x f; then echo yes; else echo no; fi",
        "ls &",
        "echo hi >&2",
        "cat <<'EOF'\n$(rm -rf /)\nEOF",
        "ls # && rm -rf /",
        "ls \\\n  -la",
        "echo \"$HOME\" $USER",
    ]);
    assert_not_read_only(&[
        "FOO=1 ls",
        "X=1",
        "/bin/ls",
        "$CMD --help",
        "l* -la",
        "cat <<EOF\n$(rm -rf /)\nEOF",
        "for f in *; do cat \"$f\"; done",
        "ls | xargs rm",
        "echo $((1 + 2))",
        "eval ls",
        "command ls",
        "> out.txt",
    ]);
}

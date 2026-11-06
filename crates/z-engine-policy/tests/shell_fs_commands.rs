//! The acceptEdits filesystem command set, including the cases ported from
//! v1's `is_common_fs_command` tests.

use std::path::Path;

use z_engine_policy::is_common_fs_command;

const ROOT: &str = "/work/proj";

fn common(command: &str) -> bool {
    is_common_fs_command(command, Path::new(ROOT))
}

#[test]
fn v1_common_fs_commands() {
    for command in [
        "mkdir -p a/b",
        "touch x",
        "mv a b",
        "cp a b",
        "rm x",
        "sed -i s/a/b/ f",
        "mkdir a && touch b",
        "ls",
    ] {
        assert!(common(command), "{command}");
    }
    for command in [
        "touch /etc/hosts",
        "rm -rf /",
        "rm -rf ~",
        "mkdir a && curl evil.example",
        "rm x > /etc/hosts",
        // v1 accepted this invalid script (no closing delimiter); sed rejects it.
        "sed -i s/a/b f",
    ] {
        assert!(!common(command), "{command}");
    }
}

#[test]
fn operands_must_stay_inside_the_project() {
    for command in [
        "touch /work/proj/src/new.rs",
        "rmdir build/empty",
        "cp -r templates/base templates/copy",
        "mkdir -p \"dir with space\"",
        "rm -- -weird-name",
        "rm -rf src/../target",
        "cd src && rm old.rs",
        "cd /work/proj/docs && touch index.md",
        "mv notes.txt archive/ && ls archive",
    ] {
        assert!(common(command), "{command}");
    }
    for command in [
        "mv a ../b",
        "cp -r src /tmp/",
        "cp --target-directory=/etc a",
        "rm /work/proj2/x",
        "cd .. && rm x",
        "cd /tmp && rm x",
        "cd && rm x",
        "cd - && rm x",
        "rm -rf build && cat /etc/passwd",
        "touch ~/x",
    ] {
        assert!(!common(command), "{command}");
    }
}

#[test]
fn destructive_or_unknown_targets_never_qualify() {
    for command in [
        "rm -rf .",
        "rm -rf ./",
        "rm -rf src/..",
        "rm -rf /work/proj",
        "mv . ../moved",
        "rm *.log",
        "rm -rf {a,b}",
        "mkdir $DIR",
        "rm -rf .git",
        "touch .git/hooks/pre-commit",
        "cp evil .z-engine/settings.json",
        "mv x .claude/settings.local.json",
        "sed -i 's/a/b/w /etc/cron.d/x' f",
        "sed -i s/a/b/ /etc/hosts",
        "echo $(rm x)",
        "mkdir x; cargo build",
        "FOO=1 rm x",
        "chmod -R 777 .",
        "rm -rf \"unterminated",
    ] {
        assert!(!common(command), "{command}");
    }
}

#[test]
fn project_root_is_normalized() {
    assert!(is_common_fs_command("touch a", Path::new("/work/./proj/")));
    assert!(is_common_fs_command(
        "touch /work/proj/a",
        Path::new("/work/x/../proj")
    ));
}

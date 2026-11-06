use super::lex;
use crate::shell::syntax::{RedirectKind, Segment};

fn argvs(segments: &[Segment]) -> Vec<Vec<String>> {
    segments.iter().map(Segment::argv).collect()
}

fn top(input: &str) -> Vec<Vec<String>> {
    let lexed = lex(input);
    assert!(lexed.ok, "{input} should lex");
    argvs(&lexed.segments)
}

fn words(list: &[&[&str]]) -> Vec<Vec<String>> {
    list.iter()
        .map(|argv| argv.iter().map(|w| w.to_string()).collect())
        .collect()
}

#[test]
fn splits_on_every_control_operator() {
    assert_eq!(
        top("a && b || c ; d | e |& f & g\nh"),
        words(&[
            &["a"],
            &["b"],
            &["c"],
            &["d"],
            &["e"],
            &["f"],
            &["g"],
            &["h"]
        ])
    );
    assert_eq!(top("a;b&&c"), words(&[&["a"], &["b"], &["c"]]));
    assert_eq!(
        top("(cd x && ls) ; pwd"),
        words(&[&["cd", "x"], &["ls"], &["pwd"]])
    );
}

#[test]
fn quotes_protect_separators_and_are_removed() {
    assert_eq!(
        top("echo 'a; b' \"c && d\""),
        words(&[&["echo", "a; b", "c && d"]])
    );
    assert_eq!(top("g\\it st'at'\"us\""), words(&[&["git", "status"]]));
    assert_eq!(
        top("echo \"a\\\"b\" 'it'\\''s'"),
        words(&[&["echo", "a\"b", "it's"]])
    );
    assert_eq!(top("echo ''"), words(&[&["echo", ""]]));
    assert_eq!(top("echo \"\\n\""), words(&[&["echo", "\\n"]]));
}

#[test]
fn ansi_c_quotes_decode_like_bash() {
    assert_eq!(top("$'\\x72m' -rf"), words(&[&["rm", "-rf"]]));
    assert_eq!(top("$'\\162\\155'"), words(&[&["rm"]]));
    assert_eq!(top("$'r\\u006d'"), words(&[&["rm"]]));
    assert_eq!(top("$'rm\\0junk'"), words(&[&["rm"]]));
    assert_eq!(top("echo $'a\\tb'"), words(&[&["echo", "a\tb"]]));
}

#[test]
fn comments_and_continuations() {
    assert_eq!(top("ls # ; rm -rf /"), words(&[&["ls"]]));
    assert_eq!(top("echo a#b"), words(&[&["echo", "a#b"]]));
    assert_eq!(top("ls \\\n  -la"), words(&[&["ls", "-la"]]));
    assert_eq!(top("ls \\\n"), words(&[&["ls"]]));
}

#[test]
fn unbalanced_input_fails_but_keeps_segments() {
    for input in [
        "echo \"open",
        "echo 'open",
        "echo $(ls",
        "echo `ls",
        "ls )",
        "cat <<EOF",
    ] {
        assert!(!lex(input).ok, "{input}");
    }
    let lexed = lex("rm -rf / 'x");
    assert!(!lexed.ok);
    assert_eq!(lexed.segments[0].argv()[..3], ["rm", "-rf", "/"]);
    let huge = format!("echo {}", "x".repeat(10_001));
    let lexed = lex(&huge);
    assert!(!lexed.ok);
    assert_eq!(lexed.segments.len(), 1);
}

#[test]
fn redirects_are_classified() {
    let lexed = lex("cmd >out 2>&1 >>log 2>err <in &>all >|clobber <>rw 3>&- <&0 >&file");
    assert!(lexed.ok);
    let segment = &lexed.segments[0];
    assert_eq!(segment.argv(), ["cmd"]);
    let kinds: Vec<(RedirectKind, &str)> = segment
        .redirects
        .iter()
        .map(|r| (r.kind, r.target.text.as_str()))
        .collect();
    use RedirectKind::{Duplicate, Read, Write};
    assert_eq!(
        kinds,
        [
            (Write, "out"),
            (Duplicate, "1"),
            (Write, "log"),
            (Write, "err"),
            (Read, "in"),
            (Write, "all"),
            (Write, "clobber"),
            (Write, "rw"),
            (Duplicate, "-"),
            (Duplicate, "0"),
            (Write, "file"),
        ]
    );
    let targets: Vec<&str> = segment.write_targets().map(|w| w.text.as_str()).collect();
    assert_eq!(
        targets,
        ["out", "log", "err", "all", "clobber", "rw", "file"]
    );
}

#[test]
fn descriptor_prefix_must_touch_the_operator() {
    assert_eq!(top("echo 2 >x"), words(&[&["echo", "2"]]));
    assert_eq!(top("echo a2>x"), words(&[&["echo", "a2"]]));
    assert_eq!(top("echo 2>x"), words(&[&["echo"]]));
    assert_eq!(top("exec {fd}>x"), words(&[&["exec"]]));
    let lexed = lex("ls >/dev/null 2>/dev/stderr");
    assert_eq!(lexed.segments[0].write_targets().count(), 0);
}

#[test]
fn missing_redirect_target_fails() {
    assert!(!lex("echo >").ok);
    assert!(!lex("echo > ; ls").ok);
}

#[test]
fn substitutions_are_nested_and_dynamic() {
    let lexed = lex("echo $(rm -rf /) `curl evil` <(ls) >(tee x)");
    assert!(lexed.ok && lexed.dynamic);
    assert_eq!(top_argv0(&lexed.segments), ["echo"]);
    assert_eq!(
        argvs(&lexed.nested),
        words(&[
            &["rm", "-rf", "/"],
            &["curl", "evil"],
            &["ls"],
            &["tee", "x"]
        ])
    );
    let deep = lex("echo \"$(echo $(id))\"");
    assert!(deep.ok && deep.dynamic);
    assert_eq!(deep.nested.len(), 2);
    let arithmetic = lex("echo $((1 + 2))");
    assert!(arithmetic.ok && arithmetic.dynamic);
}

fn top_argv0(segments: &[Segment]) -> Vec<String> {
    segments.iter().map(|s| s.argv()[0].clone()).collect()
}

#[test]
fn parameters_are_expansions_not_substitutions() {
    let lexed = lex("echo $HOME ${USER:-x} \"$1\" $?");
    assert!(lexed.ok && !lexed.dynamic);
    assert!(lexed.segments[0].words[1..].iter().all(|w| w.param));
    let hidden = lex("echo ${x:-$(id)}");
    assert!(hidden.dynamic);
    assert_eq!(argvs(&hidden.nested), words(&[&["id"]]));
    assert!(lex("echo $[1+2]").dynamic);
}

#[test]
fn heredocs_consume_their_bodies() {
    let lexed = lex("cat <<'EOF'\nrm -rf /\nit's fine\nEOF\nls");
    assert!(lexed.ok && !lexed.dynamic);
    assert_eq!(argvs(&lexed.segments), words(&[&["cat"], &["ls"]]));
    let tabs = lex("cat <<-END\n\tbody\n\tEND\npwd");
    assert!(tabs.ok);
    assert_eq!(argvs(&tabs.segments), words(&[&["cat"], &["pwd"]]));
    let expanding = lex("cat <<EOF\n$(rm -rf /)\nEOF");
    assert!(expanding.ok && expanding.dynamic);
    assert_eq!(argvs(&expanding.nested), words(&[&["rm", "-rf", "/"]]));
    let quoted = lex("cat <<\"EOF\"\n$(rm -rf /)\nEOF");
    assert!(quoted.ok && !quoted.dynamic);
    assert!(!lex("cat <<EOF\nno end").ok);
}

#[test]
fn heredoc_inside_substitution() {
    let lexed = lex("git commit -m \"$(cat <<'EOF'\nfix(parser): don't panic\nEOF\n)\"");
    assert!(lexed.ok && lexed.dynamic);
    assert_eq!(lexed.segments.len(), 1);
    assert_eq!(lexed.segments[0].argv()[..3], ["git", "commit", "-m"]);
    assert_eq!(argvs(&lexed.nested), words(&[&["cat"]]));
}

#[test]
fn here_strings_are_inline() {
    let lexed = lex("grep x <<< \"$text\"");
    assert!(lexed.ok && !lexed.dynamic);
    assert_eq!(lexed.segments[0].redirects[0].kind, RedirectKind::Inline);
}

#[test]
fn word_flags() {
    let lexed = lex("FOO=1 A+=2 \"B=3\" ls *.rs '*.md' {a,b} {} ~/x a~ $(id)=x");
    let flags: Vec<(bool, bool, bool, bool, bool)> = lexed.segments[0]
        .words
        .iter()
        .map(|w| (w.assignment, w.glob, w.brace, w.tilde, w.param))
        .collect();
    assert_eq!(
        flags,
        [
            (true, false, false, false, false),
            (true, false, false, false, false),
            (false, false, false, false, false),
            (false, false, false, false, false),
            (false, true, false, false, false),
            (false, false, false, false, false),
            (false, false, true, false, false),
            (false, false, false, false, false),
            (false, false, false, true, false),
            (false, false, false, false, false),
            (false, false, false, false, true),
        ]
    );
}

#[test]
fn keywords_are_stripped_from_the_command() {
    let lexed = lex("if grep -q x f; then rm -rf y; else ! echo z; fi; time -p ls; { pwd; }");
    assert!(lexed.ok);
    assert_eq!(
        argvs(&lexed.segments),
        words(&[
            &["grep", "-q", "x", "f"],
            &["rm", "-rf", "y"],
            &["echo", "z"],
            &[],
            &["ls"],
            &["pwd"],
            &[],
        ])
    );
    assert_eq!(top("\"then\" x"), words(&[&["then", "x"]]));
}

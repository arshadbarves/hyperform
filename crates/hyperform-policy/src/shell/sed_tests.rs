use super::*;
use crate::shell::lexer::lex;

fn call(command: &str) -> Option<(bool, Vec<String>)> {
    let lexed = lex(command);
    let words = lexed.segments[0].command();
    assert_eq!(words[0].text, "sed");
    parse_call(&words[1..]).map(|call| {
        (
            call.in_place,
            call.files.iter().map(|w| w.text.clone()).collect(),
        )
    })
}

fn safe(script: &str) -> bool {
    script_is_safe(script)
}

#[test]
fn printing_deleting_and_substituting_scripts_are_safe() {
    for script in [
        "",
        "p",
        "1,20p",
        "$d",
        "/start/,/end/p",
        "s/foo/bar/g",
        "s|/usr|/opt|2g",
        "s/a\\/b/c/I",
        "s/[a-z]*/X/",
        "s/[[:space:]]\\+/ /g",
        "1!G;h;$!d",
        "/x/{s/a/b/;p}",
        "y/abc/xyz/",
        "0~3d",
        "10q",
        "/re/I,+2d",
        "\\,^/usr,d",
        "=",
        "l",
        "#n\np",
        ":a;N;$!ba;s/\\n/ /g",
        "$a\\",
        "1i header",
    ] {
        assert!(safe(script), "{script:?} should be safe");
    }
}

#[test]
fn writing_executing_and_reading_scripts_are_not() {
    for script in [
        "w out.txt",
        "s/a/b/w out.txt",
        "s/a/b/gw out.txt",
        "s/a/b/e",
        "e rm -rf /",
        "1e date",
        "r /etc/passwd",
        "R other",
        "W out",
        "v",
        "s/a/b",
        "s/[/]/x/",
        "s/a/b/ w out",
        "/a/,/b/w out",
        "p;w out",
        ":a;w out",
        "{w out\n}",
    ] {
        assert!(!safe(script), "{script:?} should be rejected");
    }
}

#[test]
fn invocations_separate_scripts_from_files() {
    assert_eq!(
        call("sed -n 1p a b"),
        Some((false, vec!["a".into(), "b".into()]))
    );
    assert_eq!(
        call("sed -e s/a/b/ -e 1d f"),
        Some((false, vec!["f".into()]))
    );
    assert_eq!(
        call("sed --expression=p -- -f"),
        Some((false, vec!["-f".into()]))
    );
    assert_eq!(
        call("sed -nE 's/(a|b)/x/' f"),
        Some((false, vec!["f".into()]))
    );
    assert_eq!(call("sed -es/a/b/ f"), Some((false, vec!["f".into()])));
}

#[test]
fn in_place_forms_are_recognized() {
    assert_eq!(call("sed -i s/a/b/ f"), Some((true, vec!["f".into()])));
    assert_eq!(call("sed -i.bak s/a/b/ f"), Some((true, vec!["f".into()])));
    assert_eq!(call("sed -i '' s/a/b/ f"), Some((true, vec!["f".into()])));
    assert_eq!(
        call("sed -i .orig s/a/b/ f"),
        Some((true, vec!["f".into()]))
    );
    assert_eq!(
        call("sed --in-place=.bak -e 1d f"),
        Some((true, vec!["f".into()]))
    );
    assert_eq!(call("sed -I '' s/a/b/ f"), Some((true, vec!["f".into()])));
}

#[test]
fn unknown_options_script_files_and_expansions_are_rejected() {
    assert_eq!(call("sed -f script.sed f"), None);
    assert_eq!(call("sed --file=script.sed f"), None);
    assert_eq!(call("sed --zap p f"), None);
    assert_eq!(call("sed -x p f"), None);
    assert_eq!(call("sed p *.txt"), None);
    assert_eq!(call("sed \"$SCRIPT\" f"), None);
    assert_eq!(call("sed 's/a/b/w x' f"), None);
    assert_eq!(call("sed"), None);
}

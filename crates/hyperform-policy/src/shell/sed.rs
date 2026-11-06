//! `sed` invocations whose script only prints, deletes, or substitutes, so
//! the only output is standard output (or, with `-i`, the named files).

use super::sed_script::script_is_safe;
use super::syntax::Word;

#[derive(Debug)]
pub(crate) struct SedCall<'a> {
    pub in_place: bool,
    pub files: Vec<&'a Word>,
}

/// `None` unless every option is understood and the script is safe.
pub(crate) fn parse_call(args: &[Word]) -> Option<SedCall<'_>> {
    if args.iter().any(Word::splits) {
        return None;
    }
    let mut scripts: Vec<String> = Vec::new();
    let mut positionals: Vec<&Word> = Vec::new();
    let mut in_place = false;
    let mut options = true;
    let mut at = 0;
    while at < args.len() {
        let word = &args[at];
        let text = word.text.as_str();
        at += 1;
        if !options || !text.starts_with('-') || text == "-" {
            positionals.push(word);
        } else if text == "--" {
            options = false;
        } else if let Some(long) = text.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            match name {
                "quiet" | "silent" | "regexp-extended" | "separate" | "unbuffered"
                | "null-data" | "posix" | "sandbox" | "debug" | "follow-symlinks" => {}
                "expression" => match value {
                    Some(value) => scripts.push(value.to_string()),
                    None => {
                        scripts.push(args.get(at)?.text.clone());
                        at += 1;
                    }
                },
                "in-place" => in_place = true,
                "line-length" if value.is_none() => at += 1,
                "line-length" => {}
                _ => return None,
            }
        } else {
            let cluster: Vec<char> = text[1..].chars().collect();
            for (index, flag) in cluster.iter().enumerate() {
                let rest: String = cluster[index + 1..].iter().collect();
                match flag {
                    'n' | 'E' | 'r' | 's' | 'u' | 'z' => continue,
                    'e' if rest.is_empty() => {
                        scripts.push(args.get(at)?.text.clone());
                        at += 1;
                    }
                    'e' => scripts.push(rest),
                    'i' | 'I' => {
                        in_place = true;
                        // BSD `sed -i '' ..` and `-i .bak` pass the suffix separately.
                        let suffix = args
                            .get(at)
                            .is_some_and(|next| next.text.is_empty() || next.text.starts_with('.'));
                        if rest.is_empty() && suffix {
                            at += 1;
                        }
                    }
                    'l' if rest.is_empty() => at += 1,
                    'l' => {}
                    _ => return None,
                }
                break;
            }
        }
    }
    if scripts.is_empty() {
        scripts.push(positionals.first()?.text.clone());
        positionals.remove(0);
    }
    script_is_safe(&scripts.join("\n")).then_some(SedCall {
        in_place,
        files: positionals,
    })
}

#[cfg(test)]
#[path = "sed_tests.rs"]
mod tests;

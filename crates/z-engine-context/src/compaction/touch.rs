//! What a tool call touched, read from its name and input (tool names and
//! input fields follow Claude Code), and the file names a request mentions.
//! Hard keeps of compaction and of the task view are built from these.

use serde_json::Value;
use z_engine_protocol::{ContentBlock, Message, Role};

/// Tools that change the file named in their input.
const EDIT_TOOLS: [&str; 4] = ["Edit", "MultiEdit", "Write", "NotebookEdit"];
/// Tools whose failing result is check output.
const CHECK_TOOLS: [&str; 2] = ["Bash", "Verify"];
const PATH_FIELDS: [&str; 3] = ["file_path", "notebook_path", "path"];

pub fn is_edit(tool: &str) -> bool {
    EDIT_TOOLS.contains(&tool)
}

pub fn is_check(tool: &str) -> bool {
    CHECK_TOOLS.contains(&tool)
}

/// The file or directory paths a call's input names.
pub fn call_paths(input: &Value) -> Vec<String> {
    PATH_FIELDS
        .iter()
        .filter_map(|field| input.get(field)?.as_str())
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

/// A Bash call's command line.
pub fn call_command(input: &Value) -> Option<&str> {
    input.get("command")?.as_str()
}

/// Path-like words of `text`: anything with a `/` (URLs aside) or a file
/// name with a letter-led extension, without surrounding quotes or
/// punctuation. `<system-reminder>` blocks are ignored.
pub fn named_paths(text: &str) -> Vec<String> {
    let mut named: Vec<String> = Vec::new();
    let text = strip_reminders(text);
    for word in text.split(|c: char| c.is_whitespace() || "\"'`()[]{}<>,;".contains(c)) {
        let word = word.trim_end_matches(['.', ':', '!', '?']);
        let word = word.strip_prefix("./").unwrap_or(word);
        if looks_like_path(word) && !named.iter().any(|seen| seen == word) {
            named.push(word.to_string());
        }
    }
    named
}

/// `path` is one of the `named` paths: the same path, a path ending in it,
/// the same file name, or a file inside a named directory.
pub fn names_path(named: &[String], path: &str) -> bool {
    let path = path.strip_prefix("./").unwrap_or(path);
    let file_name = path.rsplit('/').next().unwrap_or(path);
    named.iter().any(|name| {
        let name = name.trim_end_matches('/');
        !name.is_empty()
            && (path == name
                || path.ends_with(&format!("/{name}"))
                || (!name.contains('/') && file_name == name)
                || path.contains(&format!("/{name}/"))
                || path.starts_with(&format!("{name}/")))
    })
}

/// The user's words in a message: its text blocks without the
/// `<system-reminder>` blocks the engine appended.
pub fn request_text(message: &Message) -> String {
    let text: Vec<String> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(strip_reminders(text)),
            _ => None,
        })
        .filter(|text| !text.trim().is_empty())
        .collect();
    text.join("\n").trim().to_string()
}

/// The newest message that opens a real user turn (no tool results).
pub fn latest_request(messages: &[Message]) -> Option<&Message> {
    messages.iter().rev().find(|message| {
        message.role == Role::User
            && !message.content.is_empty()
            && !message
                .content
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
    })
}

fn strip_reminders(text: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find(CLOSE) {
            Some(end) => &rest[start + end + CLOSE.len()..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

fn looks_like_path(word: &str) -> bool {
    if word.len() < 3 || word.contains("://") || word.starts_with('-') {
        return false;
    }
    if word.contains('/') {
        return word.chars().any(char::is_alphanumeric);
    }
    let Some((stem, extension)) = word.rsplit_once('.') else {
        return false;
    };
    stem.chars().count() >= 2
        && (1..=8).contains(&extension.len())
        && extension.starts_with(|c: char| c.is_ascii_alphabetic())
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn requests_name_paths_and_files_but_not_prose() {
        let named = named_paths(
            "Fix `src/auth.rs` and login.ts, e.g. the redirect (see ./ui/App.svelte). \
             Version 1.2 is fine; https://x.dev/a is a link.\n\
             <system-reminder>\nignore notes.md\n</system-reminder>",
        );
        assert_eq!(named, ["src/auth.rs", "login.ts", "ui/App.svelte"]);
    }

    #[test]
    fn named_paths_match_suffixes_file_names_and_directories() {
        let named = vec!["src/auth.rs".into(), "login.ts".into(), "docs/".into()];
        assert!(names_path(&named, "/repo/src/auth.rs"));
        assert!(names_path(&named, "/repo/web/login.ts"));
        assert!(names_path(&named, "/repo/docs/guide.md"));
        assert!(!names_path(&named, "/repo/src/notauth.rs"));
        assert!(!names_path(&named, "/repo/mylogin.ts"));
    }

    #[test]
    fn calls_name_their_paths_and_commands() {
        let input = json!({ "file_path": "/r/a.rs", "path": " ", "command": "cargo test" });
        assert_eq!(call_paths(&input), ["/r/a.rs"]);
        assert_eq!(call_command(&input), Some("cargo test"));
        assert!(is_edit("MultiEdit") && !is_edit("Read"));
        assert!(is_check("Bash") && !is_check("Grep"));
    }

    #[test]
    fn the_latest_request_skips_tool_result_messages() {
        let messages = vec![
            Message::user_text("first"),
            Message::new(
                Role::User,
                vec![ContentBlock::tool_result("c1".into(), "out", false)],
            ),
        ];
        let latest = latest_request(&messages).unwrap();
        assert_eq!(request_text(latest), "first");
    }
}

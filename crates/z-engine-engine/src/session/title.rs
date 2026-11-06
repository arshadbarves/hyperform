//! The sidebar title: after the first user message of a new session the
//! fast model names the task in the background.

use std::sync::Arc;

use z_engine_llm::{ModelRequest, SystemBlock};
use z_engine_prompts::auxiliary::TITLE;
use z_engine_protocol::{AgentId, Event, Message};
use z_engine_store::LogRecord;

use super::meta::write_meta;
use crate::run::side_request;
use crate::session::SessionCore;
use crate::settings::models::fast_model;

const TITLE_MAX_TOKENS: u32 = 128;
const TITLE_MAX_CHARS: usize = 80;

/// Owned by the session: cancelled when it closes.
pub(crate) fn spawn_title(core: Arc<SessionCore>, prompt: String) {
    tokio::spawn(async move {
        let cancel = core.cancel.child_token();
        let model = fast_model(&core.settings().settings, &core.main_model());
        let request = ModelRequest::new(model, vec![Message::user_text(prompt)])
            .with_system(vec![SystemBlock::new(TITLE)])
            .with_max_tokens(TITLE_MAX_TOKENS);
        match side_request(&core, &AgentId::main(), request, &cancel).await {
            Ok(turn) => {
                let title = clean(&turn.text());
                if !title.is_empty() {
                    set_title(&core, title);
                }
            }
            Err(error) => tracing::warn!(session = %core.id, %error, "title request failed"),
        }
    });
}

pub(crate) fn set_title(core: &SessionCore, title: String) {
    let record = LogRecord::Title {
        title: title.clone(),
    };
    if !core.journal.append_or_report(&record) {
        return;
    }
    core.with_state(|state| state.title = Some(title.clone()));
    core.events.emit(Event::TitleChanged { title });
    write_meta(core);
}

/// First line, without quotes, markdown emphasis or a trailing period.
fn clean(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let line = line
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '#'))
        .trim()
        .trim_end_matches('.')
        .trim();
    line.chars().take(TITLE_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_one_clean_line() {
        assert_eq!(
            clean("\n\"Fix the login loop.\"\nextra"),
            "Fix the login loop"
        );
        assert_eq!(clean("**Add JSON export**"), "Add JSON export");
        assert_eq!(clean("   "), "");
        assert_eq!(clean(&"x".repeat(200)).len(), TITLE_MAX_CHARS);
    }
}

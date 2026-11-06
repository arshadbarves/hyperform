//! Slash command helpers: running a command, waiting for its output or a
//! notice, and reading the text blocks of the message a command sent.

use z_engine_llm::ModelRequest;
use z_engine_protocol::{Command, ContentBlock, Event, Role};

use super::Harness;

impl Harness {
    pub fn command(&self, name: &str, args: &str) {
        self.send(Command::RunCommand {
            name: name.into(),
            args: args.into(),
        });
    }

    /// Markdown of the next `CommandOutput` named `wanted`.
    pub async fn command_output(&mut self, wanted: &str) -> String {
        let wanted = wanted.to_string();
        match self
            .wait(move |e| matches!(e, Event::CommandOutput { name, .. } if *name == wanted))
            .await
        {
            Event::CommandOutput { markdown, .. } => markdown,
            other => panic!("{other:?}"),
        }
    }

    /// Text of the next notice containing `needle`.
    pub async fn notice(&mut self, needle: &str) -> String {
        let needle = needle.to_string();
        match self
            .wait(move |e| matches!(e, Event::Notice { text, .. } if text.contains(&needle)))
            .await
        {
            Event::Notice { text, .. } => text,
            other => panic!("{other:?}"),
        }
    }
}

/// Text blocks of the request's last user message, in order.
pub fn user_blocks(request: &ModelRequest) -> Vec<String> {
    let message = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .expect("a user message");
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

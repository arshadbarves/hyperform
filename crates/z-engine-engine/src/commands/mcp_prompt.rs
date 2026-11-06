//! MCP prompts as commands: arguments mapped onto the declared ones, the
//! prompt fetched from its server, and its messages rendered as the body.

use z_engine_integrations::{McpPromptInfo, PromptMessage};
use z_engine_protocol::Role;

use super::args::prompt_arguments;
use crate::session::SessionCore;

/// The rendered body, or the user-facing reason it could not be built.
pub(crate) async fn render_mcp_prompt(
    core: &SessionCore,
    name: &str,
    server: &str,
    prompt: &McpPromptInfo,
    args: &str,
) -> Result<String, String> {
    let arguments = prompt_arguments(&prompt.arguments, args).map_err(|missing| {
        format!(
            "/{name} is missing required argument(s): {}. Pass them in order or as key=value.",
            missing.join(", ")
        )
    })?;
    let manager = core
        .mcp
        .manager(server)
        .ok_or_else(|| format!("the MCP server `{server}` is not running"))?;
    let messages = manager
        .get_prompt(server, &prompt.name, arguments)
        .await
        .map_err(|error| format!("/{name} failed: {error}"))?;
    Ok(render(&messages))
}

fn render(messages: &[PromptMessage]) -> String {
    messages
        .iter()
        .map(|message| match message.role {
            Role::User => message.text.trim().to_string(),
            _ => {
                let role = format!("{:?}", message.role).to_lowercase();
                format!("[{role}]\n{}", message.text.trim())
            }
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_messages_are_plain_and_others_labelled() {
        let messages = [
            PromptMessage {
                role: Role::User,
                text: "Review a.rs ".into(),
            },
            PromptMessage {
                role: Role::Assistant,
                text: "OK".into(),
            },
        ];
        assert_eq!(render(&messages), "Review a.rs\n\n[assistant]\nOK");
    }
}

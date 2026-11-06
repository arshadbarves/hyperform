//! Values exchanged with MCP servers, decoded from the 2025-06-18 schema.
//! Everything a server returns (names, descriptions, hints, content) is
//! untrusted data: it informs display and defaults, never authority.

use serde_json::Value;
use z_engine_protocol::Role;

/// The catalogs a server can announce changes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpListKind {
    Tools,
    Resources,
    Prompts,
}

/// Capabilities the server declared in its `initialize` result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub logging: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Behaviour hints declared for a tool; `None` when the server said nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolAnnotations {
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
}

impl ToolAnnotations {
    /// Spec default: tools may change their environment.
    pub fn read_only(&self) -> bool {
        self.read_only_hint.unwrap_or(false)
    }

    /// Spec default: a non-read-only tool may be destructive.
    pub fn destructive(&self) -> bool {
        !self.read_only() && self.destructive_hint.unwrap_or(true)
    }

    pub fn idempotent(&self) -> bool {
        self.idempotent_hint.unwrap_or(false)
    }

    /// Spec default: tools may reach outside systems.
    pub fn open_world(&self) -> bool {
        self.open_world_hint.unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpToolInfo {
    pub name: String,
    /// Display name: `title`, else `annotations.title`.
    pub title: Option<String>,
    pub description: Option<String>,
    /// JSON Schema of the arguments object.
    pub input_schema: Value,
    pub annotations: ToolAnnotations,
}

/// One content item of a tool result or prompt message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpContent {
    Text {
        text: String,
    },
    /// Base64 `data`.
    Image {
        data: String,
        mime_type: String,
    },
    /// Base64 `data`.
    Audio {
        data: String,
        mime_type: String,
    },
    /// A pointer to a resource the client may read.
    ResourceLink {
        uri: String,
        name: String,
    },
    /// An embedded resource; `blob` is base64.
    Resource {
        uri: String,
        mime_type: Option<String>,
        text: Option<String>,
        blob: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallToolResult {
    pub content: Vec<McpContent>,
    /// The tool itself reported failure; the call still completed.
    pub is_error: bool,
    /// `structuredContent`, when the tool declares an output schema.
    pub structured: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Contents of a read resource; `blob` is base64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceContents {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPromptInfo {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

/// A rendered prompt message; non-text content is described in `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptMessage {
    pub role: Role,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_defaults_follow_the_spec() {
        let none = ToolAnnotations::default();
        assert!(!none.read_only() && none.destructive() && !none.idempotent() && none.open_world());
        let read_only = ToolAnnotations {
            read_only_hint: Some(true),
            destructive_hint: Some(true),
            ..ToolAnnotations::default()
        };
        assert!(read_only.read_only() && !read_only.destructive());
    }
}

//! Model-facing names of MCP tools: `mcp__{server}__{tool}`, restricted to
//! the characters every provider accepts (`[A-Za-z0-9_-]`) and to 64
//! characters. Sanitizing and truncation are lossy: map a name back through
//! the manager's catalog (`McpManager::resolve_tool`) when exactness matters.

pub const MAX_TOOL_NAME_LEN: usize = 64;

const PREFIX: &str = "mcp__";
const SEPARATOR: &str = "__";
const HASH_LEN: usize = 8;

/// `mcp__{server}__{tool}`, sanitized; names longer than 64 characters keep
/// a prefix and end in `_` plus an 8-hex hash of the full sanitized name.
pub fn tool_name(server: &str, tool: &str) -> String {
    let full: String = format!("{PREFIX}{server}{SEPARATOR}{tool}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if full.len() <= MAX_TOOL_NAME_LEN {
        return full;
    }
    let keep = MAX_TOOL_NAME_LEN - HASH_LEN - 1;
    // Sanitized names are ASCII, so byte slicing is character slicing.
    format!("{}_{:08x}", &full[..keep], fnv1a(full.as_bytes()))
}

/// Splits `mcp__{server}__{tool}` at the first separator after the prefix.
/// Returns the (sanitized) parts, or `None` for names of other tools.
pub fn split_tool_name(name: &str) -> Option<(String, String)> {
    let (server, tool) = name.strip_prefix(PREFIX)?.split_once(SEPARATOR)?;
    (!server.is_empty() && !tool.is_empty()).then(|| (server.to_string(), tool.to_string()))
}

/// 32-bit FNV-1a: stable across platforms and releases.
fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |hash: u32, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_sanitizes_names() {
        assert_eq!(
            tool_name("github", "create_issue"),
            "mcp__github__create_issue"
        );
        assert_eq!(
            tool_name("my server", "fetch.url"),
            "mcp__my_server__fetch_url"
        );
        assert_eq!(tool_name("é-srv", "a/b"), "mcp___-srv__a_b");
    }

    #[test]
    fn caps_long_names_with_a_stable_hash() {
        let long_tool = "t".repeat(80);
        let name = tool_name("server", &long_tool);
        assert_eq!(name.len(), MAX_TOOL_NAME_LEN);
        assert!(name.starts_with("mcp__server__ttt"));
        assert_eq!(name, tool_name("server", &long_tool));
        let hash = &name[name.len() - HASH_LEN..];
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(&name[name.len() - HASH_LEN - 1..name.len() - HASH_LEN], "_");
        let other = tool_name("server", &format!("{long_tool}x"));
        assert_eq!(other.len(), MAX_TOOL_NAME_LEN);
        assert_ne!(name, other);
        let exact = "x".repeat(MAX_TOOL_NAME_LEN - "mcp__s__".len());
        assert_eq!(tool_name("s", &exact), format!("mcp__s__{exact}"));
    }

    #[test]
    fn splits_names_back() {
        assert_eq!(
            split_tool_name("mcp__github__create_issue"),
            Some(("github".into(), "create_issue".into()))
        );
        assert_eq!(
            split_tool_name("mcp__fs__read__file"),
            Some(("fs".into(), "read__file".into()))
        );
        assert_eq!(split_tool_name("Read"), None);
        assert_eq!(split_tool_name("mcp__only"), None);
        assert_eq!(split_tool_name("mcp____tool"), None);
        assert_eq!(split_tool_name("mcp__server__"), None);
        let (server, tool) = split_tool_name(&tool_name("srv", "echo")).unwrap();
        assert_eq!((server.as_str(), tool.as_str()), ("srv", "echo"));
    }
}

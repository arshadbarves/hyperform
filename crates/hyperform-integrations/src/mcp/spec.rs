//! How to reach one MCP server. Built by the engine from settings; header
//! and environment values are credentials and never appear in `Debug`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq)]
pub enum McpTransport {
    /// A child process speaking newline-delimited JSON-RPC on stdio.
    Stdio {
        command: String,
        args: Vec<String>,
        /// Set on top of the allowlisted environment.
        env: BTreeMap<String, String>,
        cwd: Option<PathBuf>,
    },
    /// A streamable HTTP endpoint (MCP 2025-06-18).
    Http {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

impl fmt::Debug for McpTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdio {
                command,
                args,
                env,
                cwd,
            } => f
                .debug_struct("Stdio")
                .field("command", command)
                .field("args", args)
                .field("env", &env.keys().collect::<Vec<_>>())
                .field("cwd", cwd)
                .finish(),
            Self::Http { url, headers } => f
                .debug_struct("Http")
                .field("url", url)
                .field("headers", &headers.keys().collect::<Vec<_>>())
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerSpec {
    /// Unique name; the `server` part of `mcp__{server}__{tool}`.
    pub name: String,
    pub transport: McpTransport,
    /// Deadline of every request to this server, including the handshake.
    pub timeout: Duration,
    /// Disabled servers are listed in the status but never started.
    pub enabled: bool,
}

impl McpServerSpec {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self::new(
            name,
            McpTransport::Stdio {
                command: command.into(),
                args,
                env: BTreeMap::new(),
                cwd: None,
            },
        )
    }

    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(
            name,
            McpTransport::Http {
                url: url.into(),
                headers: BTreeMap::new(),
            },
        )
    }

    fn new(name: impl Into<String>, transport: McpTransport) -> Self {
        Self {
            name: name.into(),
            transport,
            timeout: Self::DEFAULT_TIMEOUT,
            enabled: true,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_credential_values() {
        let mut spec = McpServerSpec::http("remote", "https://example.com/mcp");
        if let McpTransport::Http { headers, .. } = &mut spec.transport {
            headers.insert("Authorization".into(), "Bearer secret-token".into());
        }
        let shown = format!("{spec:?}");
        assert!(shown.contains("Authorization") && !shown.contains("secret-token"));
        let mut local = McpServerSpec::stdio("fs", "npx", vec!["-y".into()]);
        if let McpTransport::Stdio { env, .. } = &mut local.transport {
            env.insert("GITHUB_TOKEN".into(), "ghp_hidden".into());
        }
        let shown = format!("{local:?}");
        assert!(shown.contains("GITHUB_TOKEN") && !shown.contains("ghp_hidden"));
        assert!(local.enabled && local.timeout == McpServerSpec::DEFAULT_TIMEOUT);
    }
}

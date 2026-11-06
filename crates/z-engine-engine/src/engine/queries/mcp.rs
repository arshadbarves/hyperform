//! "Test" button of the MCP settings: connect to one server, list what it
//! offers, and shut it down, all within the server's timeout.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use z_engine_config::McpServerConfig;
use z_engine_integrations::{McpClient, McpServerSpec};

use crate::engine::Engine;
use crate::mcp::server_spec;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTestReport {
    pub ok: bool,
    /// Tool names as the server lists them.
    pub tools: Vec<String>,
    pub resources: usize,
    pub prompts: usize,
    pub error: Option<String>,
}

impl McpTestReport {
    fn failed(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }
}

impl Engine {
    /// Never fails: problems are reported in the result. A stdio server's
    /// relative `cwd` resolves against `project_root`.
    pub async fn test_mcp_server(
        &self,
        name: &str,
        server: &McpServerConfig,
        project_root: Option<&Path>,
    ) -> McpTestReport {
        let spec = match spec(name, server, project_root, self.paths().home_dir.as_deref()) {
            Ok(built) => built,
            Err(error) => return McpTestReport::failed(error),
        };
        let limit = spec.timeout;
        let client = match tokio::time::timeout(limit, McpClient::connect(&spec, None)).await {
            Ok(Ok(client)) => client,
            Ok(Err(error)) => return McpTestReport::failed(error.to_string()),
            Err(_) => return McpTestReport::failed(timed_out(limit)),
        };
        let listed = tokio::time::timeout(limit, async {
            let tools = client.list_tools().await?;
            let resources = client.list_resources().await?;
            let prompts = client.list_prompts().await?;
            Ok::<_, z_engine_integrations::IntegrationError>((tools, resources, prompts))
        })
        .await;
        client.shutdown().await;
        match listed {
            Ok(Ok((tools, resources, prompts))) => McpTestReport {
                ok: true,
                tools: tools.into_iter().map(|tool| tool.name).collect(),
                resources: resources.len(),
                prompts: prompts.len(),
                error: None,
            },
            Ok(Err(error)) => McpTestReport::failed(error.to_string()),
            Err(_) => McpTestReport::failed(timed_out(limit)),
        }
    }
}

fn timed_out(limit: Duration) -> String {
    format!("the server did not answer within {} s", limit.as_secs())
}

/// The session's spec builder; a nameless or disabled entry still tests.
fn spec(
    name: &str,
    server: &McpServerConfig,
    project_root: Option<&Path>,
    home: Option<&Path>,
) -> Result<McpServerSpec, String> {
    let name = match name.trim() {
        "" => "test",
        name => name,
    };
    let mut spec = server_spec(name, server, project_root, home)?;
    spec.enabled = true;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use z_engine_integrations::McpTransport;

    use super::super::testing::engine;
    use super::*;

    fn stdio(command: &str) -> McpServerConfig {
        McpServerConfig {
            command: Some(command.to_string()),
            cwd: Some("tools".to_string()),
            timeout_secs: 5,
            ..McpServerConfig::default()
        }
    }

    #[test]
    fn specs_resolve_the_cwd_against_the_project() {
        let local = spec("fs", &stdio("npx"), Some(Path::new("/work/app")), None).unwrap();
        let McpTransport::Stdio { command, cwd, .. } = &local.transport else {
            panic!("stdio expected");
        };
        assert_eq!(command, "npx");
        assert_eq!(cwd.as_deref(), Some(Path::new("/work/app/tools")));
        assert_eq!(local.timeout, Duration::from_secs(5));
        let http = McpServerConfig {
            url: Some("https://example.com/mcp".into()),
            ..McpServerConfig::default()
        };
        assert!(matches!(
            spec("", &http, None, None).unwrap().transport,
            McpTransport::Http { .. }
        ));
        let neither = McpServerConfig::default();
        assert!(spec("x", &neither, None, None).is_err());
    }

    #[tokio::test]
    async fn failures_are_reported_not_raised() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let report = engine
            .test_mcp_server("x", &McpServerConfig::default(), None)
            .await;
        assert!(!report.ok && report.error.is_some());
        let missing = stdio("zengine-no-such-mcp-server");
        let report = engine
            .test_mcp_server("x", &missing, Some(dir.path()))
            .await;
        assert!(!report.ok, "{report:?}");
        assert!(report.error.is_some());
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["ok"], false);
        assert!(json.get("resources").is_some() && json.get("prompts").is_some());
    }
}

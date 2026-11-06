//! `[mcp.servers]` entries as manager specs: stdio (command, args, env,
//! cwd relative to the project root) or streamable HTTP (url, headers).

use std::path::Path;
use std::time::Duration;

use z_engine_config::{McpServerConfig, McpSettings};
use z_engine_host::{expand_tilde, resolve};
use z_engine_integrations::{McpServerSpec, McpTransport};

/// One server to run, with the tools hidden from the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerPlan {
    pub spec: McpServerSpec,
    pub disabled_tools: Vec<String>,
}

/// Plans in name order, plus one message per unusable entry.
pub(super) fn server_plans(
    settings: &McpSettings,
    root: &Path,
    home: Option<&Path>,
) -> (Vec<ServerPlan>, Vec<String>) {
    let mut plans = Vec::new();
    let mut problems = Vec::new();
    for (name, config) in &settings.servers {
        match server_spec(name, config, Some(root), home) {
            Ok(spec) => plans.push(ServerPlan {
                spec,
                disabled_tools: config.disabled_tools.clone(),
            }),
            Err(problem) => problems.push(format!("MCP server `{name}` is skipped: {problem}")),
        }
    }
    (plans, problems)
}

/// One server's spec. A stdio server runs in its `cwd` resolved against
/// `root`, else in `root` itself (the process's directory without one).
pub(crate) fn server_spec(
    name: &str,
    config: &McpServerConfig,
    root: Option<&Path>,
    home: Option<&Path>,
) -> Result<McpServerSpec, String> {
    if name.trim().is_empty() {
        return Err("the server needs a name".to_string());
    }
    if let Some(problem) = config.transport_error() {
        return Err(problem.to_string());
    }
    let transport = match (&config.command, &config.url) {
        (Some(command), _) if !command.trim().is_empty() => McpTransport::Stdio {
            command: command.trim().to_string(),
            args: config.args.clone(),
            env: config.env.clone(),
            cwd: match (config.cwd.as_deref().map(str::trim), root) {
                (Some(cwd), Some(root)) if !cwd.is_empty() => {
                    Some(resolve(root, expand_tilde(cwd, home)))
                }
                (Some(cwd), None) if !cwd.is_empty() => Some(expand_tilde(cwd, home)),
                (_, root) => root.map(Path::to_path_buf),
            },
        },
        (_, Some(url)) => McpTransport::Http {
            url: url.trim().to_string(),
            headers: config.headers.clone(),
        },
        _ => return Err("set `command` or `url`".to_string()),
    };
    Ok(McpServerSpec {
        name: name.trim().to_string(),
        transport,
        timeout: Duration::from_secs(config.timeout_secs.max(1)),
        enabled: config.enabled,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn stdio_and_http_servers_become_specs() {
        let mut settings = McpSettings::default();
        settings.servers.insert(
            "fs".into(),
            McpServerConfig {
                command: Some(" npx ".into()),
                args: vec!["-y".into()],
                cwd: Some("tools".into()),
                disabled_tools: vec!["delete".into()],
                timeout_secs: 0,
                ..McpServerConfig::default()
            },
        );
        settings.servers.insert(
            "remote".into(),
            McpServerConfig {
                url: Some("https://example.com/mcp".into()),
                enabled: false,
                ..McpServerConfig::default()
            },
        );
        settings
            .servers
            .insert("broken".into(), McpServerConfig::default());
        let (plans, problems) = server_plans(&settings, Path::new("/work/app"), None);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("broken"));
        let fs = plans.iter().find(|plan| plan.spec.name == "fs").unwrap();
        let McpTransport::Stdio { command, cwd, .. } = &fs.spec.transport else {
            panic!("stdio expected");
        };
        assert_eq!(command, "npx");
        assert_eq!(
            cwd.as_deref(),
            Some(PathBuf::from("/work/app/tools").as_path())
        );
        assert_eq!(fs.spec.timeout, Duration::from_secs(1));
        assert_eq!(fs.disabled_tools, ["delete"]);
        let remote = plans
            .iter()
            .find(|plan| plan.spec.name == "remote")
            .unwrap();
        assert!(!remote.spec.enabled);
        assert!(matches!(remote.spec.transport, McpTransport::Http { .. }));
    }
}

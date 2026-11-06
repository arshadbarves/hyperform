//! Effective per-session settings: the layered load, workspace trust, and
//! everything derived from settings once per (re)load (extensions,
//! instruction files, shell, environment policy, extra directories).

use std::path::{Path, PathBuf};

use hyperform_config::{
    Credentials, DecisionSettings, EnvOverrides, Extensions, MergePolicy, Paths,
    SearchBackend as ConfigSearch, Settings, TrustStore, discover_extensions, load_with_env,
    resolve_search_key,
};
use hyperform_context::InstructionDoc;
use hyperform_host::{EnvPolicy, SearchBackend, ShellSpec, expand_tilde, resolve, resolve_shell};
use hyperform_protocol::NoticeLevel;

use super::instructions::instruction_docs;

/// Settings as one session applies them. Shared as `Arc` snapshots: a
/// reload swaps the whole value, so a running round sees one version.
#[derive(Debug, Clone)]
pub(crate) struct SessionSettings {
    /// When untrusted, everything that runs code or loosens permissions
    /// comes from the user layer (see `restrict_to_user_level`).
    pub settings: Settings,
    pub trusted: bool,
    /// What an untrusted project set that is ignored (`hooks`,
    /// `permission rules and mode`, `shell and sandbox`, ...); empty when
    /// trusted.
    pub withheld: Vec<String>,
    pub extensions: Extensions,
    pub instructions: Vec<InstructionDoc>,
    pub shell: ShellSpec,
    pub env: EnvPolicy,
    pub additional_dirs: Vec<PathBuf>,
    /// The configured web search backend with its key.
    pub web_search: SearchBackend,
}

/// A load plus the notices it produced for the GUI.
#[derive(Debug)]
pub(crate) struct LoadReport {
    pub settings: SessionSettings,
    pub notices: Vec<(NoticeLevel, String)>,
}

pub(crate) fn load_session_settings(paths: &Paths, root: &Path, env: &EnvOverrides) -> LoadReport {
    let loaded = load_with_env(paths, Some(root), env);
    let mut notices: Vec<(NoticeLevel, String)> = loaded
        .warnings
        .iter()
        .map(|warning| (NoticeLevel::Warn, warning.clone()))
        .collect();
    let mut settings = loaded.settings;
    let trusted = match TrustStore::load(&paths.trust_file) {
        Ok(store) => store.is_trusted(root),
        Err(error) => {
            notices.push((
                NoticeLevel::Warn,
                format!("workspace trust unavailable: {error}"),
            ));
            false
        }
    };
    let withheld = if trusted {
        Vec::new()
    } else {
        restrict_to_user_level(paths, env, &mut settings)
    };
    if !withheld.is_empty() {
        notices.push((
            NoticeLevel::Warn,
            format!(
                "This workspace is not trusted: project settings for {} are ignored until you \
                 trust it. Its deny and ask rules still apply.",
                withheld.join(", ")
            ),
        ));
    }
    if let Some(reason) = super::sandbox::unavailable_reason(&settings) {
        notices.push((NoticeLevel::Warn, reason));
    }
    let compat = settings.compat.claude;
    let extensions = discover_extensions(paths, root, compat);
    notices.extend(extensions.errors.iter().map(|error| {
        let text = format!("skipped {}: {}", error.path, error.message);
        (NoticeLevel::Warn, text)
    }));
    let instructions = instruction_docs(paths, root, compat, &extensions);
    let shell = resolve_shell(settings.shell.path.as_deref());
    let env = EnvPolicy {
        passthrough: settings.shell.env_passthrough.clone(),
        extra: settings.shell.env.clone(),
    };
    let additional_dirs = settings
        .permissions
        .additional_directories
        .iter()
        .map(|dir| resolve(root, expand_tilde(dir, paths.home_dir.as_deref())))
        .collect();
    let web_search = web_search(paths, &settings, &mut notices);
    LoadReport {
        settings: SessionSettings {
            settings,
            trusted,
            withheld,
            extensions,
            instructions,
            shell,
            env,
            additional_dirs,
            web_search,
        },
        notices,
    }
}

/// The search backend with its key; a backend without a key is disabled
/// with a notice.
fn web_search(
    paths: &Paths,
    settings: &Settings,
    notices: &mut Vec<(NoticeLevel, String)>,
) -> SearchBackend {
    let backend = settings.web.search_backend;
    let keyed: fn(String) -> SearchBackend = match backend {
        ConfigSearch::Disabled => return SearchBackend::None,
        ConfigSearch::Searxng => {
            return match settings.web.search_url.as_deref().map(str::trim) {
                Some(url) if !url.is_empty() => SearchBackend::Searxng {
                    base_url: url.to_string(),
                },
                _ => SearchBackend::None,
            };
        }
        ConfigSearch::Brave => |api_key| SearchBackend::Brave { api_key },
        ConfigSearch::Tavily => |api_key| SearchBackend::Tavily { api_key },
        ConfigSearch::Exa => |api_key| SearchBackend::Exa { api_key },
    };
    // An unreadable key file is reported with the model client's warning.
    let key = Credentials::load(&paths.auth_file)
        .ok()
        .and_then(|credentials| resolve_search_key(&credentials, backend));
    match key {
        Some(api_key) => keyed(api_key),
        None => {
            notices.push((
                NoticeLevel::Warn,
                "web search is configured without an API key; it stays off".to_string(),
            ));
            SearchBackend::None
        }
    }
}

/// An untrusted project may only make things stricter: everything that
/// runs code, loosens permissions or redirects traffic comes from the user
/// layer, while the project's `deny` and `ask` rules, its `planning` when
/// it turns plan mode on, its `work.merge = "ask"`, and the serial paths it
/// adds to the user's (never removes), still apply. Returns what the
/// project had set differently.
fn restrict_to_user_level(
    paths: &Paths,
    env: &EnvOverrides,
    settings: &mut Settings,
) -> Vec<String> {
    let user = load_with_env(paths, None, env).settings;
    let withheld = trust_gated(settings, &user);
    settings.hooks = user.hooks;
    settings.mcp = user.mcp;
    settings.verification.checks = user.verification.checks;
    settings.permissions.mode = user.permissions.mode;
    settings.permissions.planning |= user.permissions.planning;
    settings.permissions.allow = user.permissions.allow;
    settings.permissions.additional_directories = user.permissions.additional_directories;
    settings.permissions.auto_allow_read_only_bash = user.permissions.auto_allow_read_only_bash;
    settings.shell = user.shell;
    settings.provider = user.provider;
    settings.web = user.web;
    settings.lsp = user.lsp;
    settings.decisions = user.decisions;
    if settings.work.merge == MergePolicy::Auto {
        settings.work.merge = user.work.merge;
    }
    for path in user.work.serial_paths {
        if !settings.work.serial_paths.contains(&path) {
            settings.work.serial_paths.push(path);
        }
    }
    withheld
}

/// What `project` (all layers merged) sets differently from the user layer
/// among the settings only a trusted project may supply, named for people
/// (`hooks`, `permission rules and mode`, ...).
pub(crate) fn trust_gated(project: &Settings, user: &Settings) -> Vec<String> {
    let permissions = &project.permissions;
    let loosened = permissions.mode != user.permissions.mode
        || (user.permissions.planning && !permissions.planning)
        || permissions.allow != user.permissions.allow
        || permissions.additional_directories != user.permissions.additional_directories
        || permissions.auto_allow_read_only_bash != user.permissions.auto_allow_read_only_bash;
    let differs = [
        ("hooks", project.hooks != user.hooks),
        ("MCP servers", project.mcp != user.mcp),
        (
            "checks",
            project.verification.checks != user.verification.checks,
        ),
        ("permission rules and mode", loosened),
        ("shell and sandbox", project.shell != user.shell),
        ("provider", project.provider != user.provider),
        ("web access", project.web != user.web),
        ("language servers", project.lsp != user.lsp),
        (
            "decision model",
            without_rules(project) != without_rules(user),
        ),
        (
            "decision rules",
            project.decisions.rules != user.decisions.rules,
        ),
        (
            "automatic merges",
            project.work.merge == MergePolicy::Auto && user.work.merge != MergePolicy::Auto,
        ),
        (
            "fewer serial paths",
            user.work
                .serial_paths
                .iter()
                .any(|path| !project.work.serial_paths.contains(path)),
        ),
    ];
    differs
        .into_iter()
        .filter(|(_, differs)| *differs)
        .map(|(what, _)| what.to_string())
        .collect()
}

fn without_rules(settings: &Settings) -> DecisionSettings {
    DecisionSettings {
        rules: Vec::new(),
        ..settings.decisions.clone()
    }
}

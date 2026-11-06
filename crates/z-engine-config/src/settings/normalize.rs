//! Post-merge normalization: clamp ranges, drop what cannot run, and warn about each change.

use super::{
    DEFAULT_BASE_URL, DEFAULT_MODEL, DEFAULT_PET_NAME, MAX_COMPACT_AT_PERCENT, MAX_CONTINUATIONS,
    MAX_OUTPUT_TOKENS, MAX_PET_NAME_CHARS, MIN_COMPACT_AT_PERCENT, MIN_OUTPUT_TOKENS,
    SearchBackend, Settings, is_hook_event, normalize_decisions, normalize_experimental,
    normalize_work,
};

pub(crate) fn normalize(settings: &mut Settings) -> Vec<String> {
    let mut warnings = Vec::new();
    let w = &mut warnings;
    if settings.model.main.trim().is_empty() {
        w.push(format!("model.main is empty; using {DEFAULT_MODEL}"));
        settings.model.main = DEFAULT_MODEL.to_string();
    }
    let ranges = [
        (
            "model.max_output_tokens",
            &mut settings.model.max_output_tokens,
            MIN_OUTPUT_TOKENS..=MAX_OUTPUT_TOKENS,
        ),
        (
            "context.compact_at_percent",
            &mut settings.context.compact_at_percent,
            MIN_COMPACT_AT_PERCENT..=MAX_COMPACT_AT_PERCENT,
        ),
        (
            "verification.max_continuations",
            &mut settings.verification.max_continuations,
            0..=MAX_CONTINUATIONS,
        ),
        (
            "agents.max_concurrent",
            &mut settings.agents.max_concurrent,
            1..=u32::MAX,
        ),
        (
            "agents.max_turns",
            &mut settings.agents.max_turns,
            1..=u32::MAX,
        ),
    ];
    for (key, value, range) in ranges {
        let clamped = (*value).clamp(*range.start(), *range.end());
        if clamped != *value {
            w.push(format!("{key} = {value} is out of range; using {clamped}"));
            *value = clamped;
        }
    }
    let cap = &mut settings.agents.session_cost_cap_usd;
    if !cap.is_finite() || *cap < 0.0 {
        w.push(format!(
            "agents.session_cost_cap_usd = {cap} is invalid; the cap is off"
        ));
        *cap = 0.0;
    }
    let base_url = settings.provider.base_url.trim().trim_end_matches('/');
    settings.provider.base_url = match base_url {
        "" => DEFAULT_BASE_URL.to_string(),
        url => url.to_string(),
    };
    let web = &settings.web;
    let has_url = web
        .search_url
        .as_deref()
        .is_some_and(|url| !url.trim().is_empty());
    if web.search_backend == SearchBackend::Searxng && !has_url {
        w.push("web.search_backend = \"searxng\" needs web.search_url".to_string());
    }
    normalize_hooks(settings, w);
    normalize_checks(settings, w);
    normalize_servers(settings, w);
    normalize_sandbox(settings, w);
    normalize_pet_name(settings, w);
    normalize_experimental(settings, w);
    normalize_decisions(&mut settings.decisions, w);
    normalize_work(&mut settings.work, w);
    warnings
}

fn normalize_pet_name(settings: &mut Settings, w: &mut Vec<String>) {
    let name = &mut settings.ui.pet.name;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        *name = DEFAULT_PET_NAME.to_string();
        return;
    }
    if trimmed.chars().count() > MAX_PET_NAME_CHARS {
        let short: String = trimmed.chars().take(MAX_PET_NAME_CHARS).collect();
        w.push(format!(
            "ui.pet.name is longer than {MAX_PET_NAME_CHARS} characters; using \"{short}\""
        ));
        *name = short;
    } else if trimmed.len() != name.len() {
        *name = trimmed.to_string();
    }
}

fn normalize_sandbox(settings: &mut Settings, w: &mut Vec<String>) {
    let sandbox = &mut settings.shell.sandbox;
    sandbox.extra_writable.retain(|entry| {
        warn_unless(w, !entry.trim().is_empty(), || {
            "shell.sandbox.extra_writable: an empty entry was skipped".to_string()
        })
    });
    for entry in &mut sandbox.extra_writable {
        *entry = entry.trim().to_string();
    }
    let broad = sandbox
        .extra_writable
        .iter()
        .filter(|entry| matches!(entry.trim_end_matches(['/', '\\']), "" | "~"));
    for entry in broad {
        w.push(format!(
            "shell.sandbox.extra_writable: \"{entry}\" makes most of the disk writable inside the sandbox"
        ));
    }
}

fn normalize_hooks(settings: &mut Settings, w: &mut Vec<String>) {
    for (event, hooks) in &mut settings.hooks {
        if !is_hook_event(event) {
            w.push(format!(
                "hooks.{event} is not a hook event; its hooks never run"
            ));
        }
        hooks.retain(|hook| {
            let ok = !hook.command.trim().is_empty();
            warn_unless(w, ok, || {
                format!("hooks.{event}: a hook without `command` was skipped")
            })
        });
        for hook in hooks.iter_mut() {
            positive_timeout(w, &format!("hooks.{event}"), &mut hook.timeout_secs);
        }
    }
    settings.hooks.retain(|_, hooks| !hooks.is_empty());
}

fn normalize_checks(settings: &mut Settings, w: &mut Vec<String>) {
    let checks = &mut settings.verification.checks;
    checks.retain(|check| {
        let missing = [("id", &check.id), ("command", &check.command)]
            .into_iter()
            .find_map(|(field, value)| value.trim().is_empty().then_some(field));
        warn_unless(w, missing.is_none(), || {
            let field = missing.unwrap_or_default();
            format!("verification.checks: a check without `{field}` was skipped")
        })
    });
    for check in checks.iter_mut() {
        if check.label.trim().is_empty() {
            check.label = check.id.clone();
        }
        let key = format!("verification.checks.{}", check.id);
        positive_timeout(w, &key, &mut check.timeout_secs);
    }
}

fn normalize_servers(settings: &mut Settings, w: &mut Vec<String>) {
    settings.mcp.servers.retain(|name, server| {
        let problem = server.transport_error();
        warn_unless(w, problem.is_none(), || {
            let problem = problem.unwrap_or_default();
            format!("mcp.servers.{name}: {problem}; server skipped")
        })
    });
    for (name, server) in &mut settings.mcp.servers {
        positive_timeout(w, &format!("mcp.servers.{name}"), &mut server.timeout_secs);
    }
    // `enabled = false` without a command switches a built-in server off.
    settings.lsp.servers.retain(|name, server| {
        let ok = !server.enabled || !server.command.trim().is_empty();
        warn_unless(w, ok, || {
            format!("lsp.servers.{name}: set `command`; server skipped")
        })
    });
}

fn warn_unless(w: &mut Vec<String>, ok: bool, warning: impl FnOnce() -> String) -> bool {
    if !ok {
        w.push(warning());
    }
    ok
}

fn positive_timeout(w: &mut Vec<String>, owner: &str, timeout_secs: &mut u64) {
    if *timeout_secs == 0 {
        w.push(format!("{owner}.timeout_secs = 0 is out of range; using 1"));
        *timeout_secs = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{CheckConfig, HookConfig, LspServerConfig, McpServerConfig};

    #[test]
    fn defaults_need_no_adjustment() {
        let mut settings = Settings::default();
        assert!(normalize(&mut settings).is_empty());
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn out_of_range_values_are_clamped_with_warnings() {
        let mut settings = Settings::default();
        settings.model.max_output_tokens = 10;
        settings.context.compact_at_percent = 150;
        settings.verification.max_continuations = 50;
        settings.agents.max_concurrent = 0;
        settings.agents.session_cost_cap_usd = -1.0;
        settings.provider.base_url = "http://localhost:8080/v1///".into();
        let warnings = normalize(&mut settings);
        assert_eq!(settings.model.max_output_tokens, 256);
        assert_eq!(settings.context.compact_at_percent, 99);
        assert_eq!(settings.verification.max_continuations, 10);
        assert_eq!(settings.agents.max_concurrent, 1);
        assert_eq!(settings.agents.session_cost_cap_usd, 0.0);
        assert_eq!(settings.provider.base_url, "http://localhost:8080/v1");
        assert_eq!(warnings.len(), 5, "{warnings:?}");
    }

    #[test]
    fn unrunnable_entries_are_dropped() {
        let mut settings = Settings::default();
        let hook = HookConfig::default();
        settings
            .hooks
            .insert("PreToolUse".into(), vec![hook.clone()]);
        let timed = HookConfig {
            command: "echo".into(),
            timeout_secs: 0,
            ..hook
        };
        settings.hooks.insert("BeforeLunch".into(), vec![timed]);
        let both = McpServerConfig {
            command: Some("npx".into()),
            url: Some("http://x".into()),
            ..McpServerConfig::default()
        };
        settings.mcp.servers.insert("both".into(), both);
        settings
            .lsp
            .servers
            .insert("empty".into(), LspServerConfig::default());
        let check = CheckConfig {
            id: "t".into(),
            ..CheckConfig::default()
        };
        settings.verification.checks.push(check);
        let warnings = normalize(&mut settings);
        assert!(!settings.hooks.contains_key("PreToolUse"));
        assert_eq!(settings.hooks["BeforeLunch"][0].timeout_secs, 1);
        assert!(settings.mcp.servers.is_empty() && settings.lsp.servers.is_empty());
        assert!(settings.verification.checks.is_empty());
        assert_eq!(warnings.len(), 6, "{warnings:?}");
    }

    #[test]
    fn a_disabled_language_server_needs_no_command() {
        let mut settings = Settings::default();
        let off = LspServerConfig {
            enabled: false,
            ..LspServerConfig::default()
        };
        settings.lsp.servers.insert("clangd".into(), off);
        let warnings = normalize(&mut settings);
        assert!(settings.lsp.servers.contains_key("clangd"));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn the_pet_name_is_trimmed_clamped_and_never_empty() {
        let mut settings = Settings::default();
        settings.ui.pet.name = "  Pip  ".into();
        assert!(normalize(&mut settings).is_empty());
        assert_eq!(settings.ui.pet.name, "Pip");

        settings.ui.pet.name = "   ".into();
        assert!(normalize(&mut settings).is_empty());
        assert_eq!(settings.ui.pet.name, "Zen");

        settings.ui.pet.name = "Ω".repeat(30);
        let warnings = normalize(&mut settings);
        assert_eq!(settings.ui.pet.name.chars().count(), 24);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn sandbox_entries_are_trimmed_and_broad_ones_flagged() {
        let mut settings = Settings::default();
        settings.shell.sandbox.extra_writable =
            vec![" out ".into(), "  ".into(), "/".into(), "~/".into()];
        let warnings = normalize(&mut settings);
        assert_eq!(settings.shell.sandbox.extra_writable, ["out", "/", "~/"]);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings[1].contains("\"/\" makes most of the disk writable"));
    }
}
